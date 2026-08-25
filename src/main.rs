use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rusqlite::Connection;

mod backend;
mod display;

use backend::config::{Config, ConfigDir, expand_tilde, read_config, set_new_path};
use backend::database::{create_sqlite_db, get_db};
use backend::import::import;
use backend::migrate::run_migrate_command;
use backend::wipe::wipe_tasks;

use display::theme::{Theme, create_empty_theme_toml, migrate_theme, read_theme};
use display::tui::{LayoutView, run_tui};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Will run checklist off of a memory SQLite database.
    /// As a result, no data will be kept on program exit.
    #[arg(short, long)]
    memory: bool,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Provide where you want the config_dir that holds checklist's
    /// data files to be (config, database, theme) instead of the
    /// default location. This only holds true for that particular run.
    #[arg(long, global = true, value_parser = expand_tilde)]
    config_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initializes checklist, creating a SQLite database
    /// that the program will automatically use
    /// and a config json file
    Init {
        /// Optional argument that will set a given
        /// SQLite database as the new default
        #[arg(short, long, value_parser = expand_tilde)]
        set: Option<PathBuf>,
    },

    /// Wipe tasks in the database
    Wipe {
        /// Bypass confirmation check
        #[arg(short)]
        yes: bool,

        /// Pass in to drop the 'task' table entirely.
        /// Use with caution.
        #[arg(long)]
        hard: bool,
    },

    /// Displays tasks in an interactive terminal
    Display {
        /// What Layout View to start with
        #[arg(short, long, value_enum)]
        view: Option<LayoutView>,
    },

    /// Tells you where checklist files are stored
    Where {
        /// Gives you the full path to the SQLite database
        #[arg(short, long)]
        db: bool,

        /// Gives you the full path to the configuration file
        #[arg(short, long)]
        config: bool,

        /// Gives you the full path to the theme.toml file
        #[arg(short, long)]
        theme: bool,
    },

    /// Import tasks from one checklist db to your current one
    Import {
        /// Path to the database you want to import
        #[arg(value_parser = expand_tilde)]
        database: PathBuf,

        /// Open the TUI displaying the imported tasks after the import finishes
        #[arg(long)]
        display: bool,

        /// What Layout View to start with (used with --display)
        #[arg(short, long, value_enum)]
        view: Option<LayoutView>,
    },

    /// Manage the theme.toml file
    Theme {
        /// Re-serialize theme.toml with all current keys and defaults.
        /// Useful for picking up newly available theme options after an
        /// update. Note: comments and custom formatting are not preserved.
        #[arg(long)]
        migrate: bool,
    },

    /// Inspect or move the database schema version. Moving down produces a
    /// database readable by the checklist release that shipped with that
    /// schema version — useful when sharing a database with an older install.
    Migrate {
        /// Move to this exact schema version (may go down or up).
        #[arg(long, conflicts_with_all = ["prior", "latest"])]
        to: Option<usize>,

        /// Move back one schema version from the current one.
        #[arg(long, conflicts_with_all = ["to", "latest"])]
        prior: bool,

        /// Upgrade to the latest schema version this build supports.
        #[arg(long, conflicts_with_all = ["to", "prior"])]
        latest: bool,

        /// Skip the confirmation prompt (only asked when moving down).
        #[arg(short, long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Resolve the config directory once and thread it through. This is the
    // single source of truth for where checklist's data files live.
    let dir = ConfigDir::resolve_config_dir(cli.config_dir)?;

    match cli.command {
        Some(Commands::Init { set }) => {
            if let Some(valid_path) = set {
                set_new_path(valid_path, &dir)?;
            } else {
                // Probably need to decouple, but this will make the config
                // file and the sqlite db
                create_sqlite_db(&dir)?;
                println!("Successfully created the database to store your items in!");
            }

            // This will handle the theme, making a default one if
            // one doesn't exist. Migrate it right away so a fresh theme
            // file has all the current keys/options shown.
            let theme_path = dir.theme_path();
            if !theme_path.exists() {
                create_empty_theme_toml(&dir)?;
                migrate_theme(&dir)?;
            }
        }

        Some(Commands::Wipe { yes, hard }) => {
            let conn = get_db(cli.memory, &dir)?;
            wipe_tasks(&conn, yes, hard)?
        }

        Some(Commands::Display { view }) => bootstrap(cli.memory, dir, view)?,

        Some(Commands::Where { db, config, theme }) => {
            if !db && !config && !theme {
                println!("{}", dir.path().display());
            }
            if db {
                match read_config(&dir) {
                    Ok(config) => {
                        let db_path = config.db_path;
                        if db_path.exists() {
                            println!("{}", db_path.display());
                        } else {
                            eprintln!("Could not find a SQLite database file.")
                        }
                    }
                    Err(_) => {
                        eprintln!("Could not read the config file holding the database location.");
                    }
                }
            }
            if config {
                let config_path = dir.config_path();
                if config_path.exists() {
                    println!("{}", config_path.display());
                } else {
                    eprintln!("Could not find a config file.")
                }
            }
            if theme {
                let theme_path = dir.theme_path();
                if theme_path.exists() {
                    println!("{}", theme_path.display());
                } else {
                    eprintln!("Could not find a theme file.")
                }
            }
        }

        Some(Commands::Import {
            database,
            display,
            view,
        }) => {
            let conn = import(database, cli.memory, &dir)?;
            if display {
                launch_tui(cli.memory, dir, conn, view)?;
            }
        }

        Some(Commands::Theme { migrate }) => {
            if migrate {
                migrate_theme(&dir)?;
            } else {
                eprintln!(
                    "No action specified. Use `checklist theme --migrate` to re-serialize theme.toml."
                );
            }
        }

        Some(Commands::Migrate {
            to,
            prior,
            latest,
            yes,
        }) => {
            run_migrate_command(&dir, cli.memory, to, prior, latest, yes)?;
        }

        None => {
            bootstrap(cli.memory, dir, Some(LayoutView::default()))?;
        }
    }

    Ok(())
}

fn bootstrap(memory: bool, dir: ConfigDir, view: Option<LayoutView>) -> Result<()> {
    let conn = get_db(memory, &dir).or_else(|e| {
        eprintln!("Error retrieving database: {}", e);
        // Disk mode with no config yet: bootstrap a default DB + config, then retry.
        if !memory {
            create_sqlite_db(&dir)?;
            println!("Successfully created the database to store your items in!");
        }
        get_db(memory, &dir)
    })?;
    launch_tui(memory, dir, conn, view)
}

fn launch_tui(
    memory: bool,
    dir: ConfigDir,
    conn: Connection,
    view: Option<LayoutView>,
) -> Result<()> {
    let config = match read_config(&dir) {
        Ok(config) => config,
        Err(_) if memory => Config::new(PathBuf::new()),
        Err(e) => {
            eprintln!("Error reading config: {}", e);
            create_sqlite_db(&dir)?;
            println!("Successfully created the database to store your items in!");
            read_config(&dir)?
        }
    };

    // In memory mode, use a default theme without touching disk so no
    // theme.toml is created. Otherwise, make a default one if it doesn't
    // exist and read it in.
    let theme = if memory {
        Theme::default()
    } else {
        let theme_path = dir.theme_path();
        if !theme_path.exists() {
            create_empty_theme_toml(&dir)?;
            migrate_theme(&dir)?; // if a brand new theme, let's save contents for new users
        }
        read_theme(&dir)?
    };

    run_tui(memory, conn, dir, config, theme, view)?;
    Ok(())
}
