use std::path::PathBuf;

use anyhow::Result;
use backend::import::import_database;
use clap::{Parser, Subcommand};

mod backend;
mod display;

use backend::config::{ConfigDir, expand_tilde, read_config, set_new_path};
use backend::database::{create_sqlite_db, get_db};
use backend::wipe::wipe_tasks;

use display::theme::{create_empty_theme_toml, read_theme};
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
        #[arg(short, long, value_parser = expand_tilde)]
        database: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Resolve the config directory once and thread it through. This is the
    // single source of truth for where checklist's data files live.
    let dir = ConfigDir::resolve_default()?;

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
            // One doesn't exist
            let theme_path = dir.theme_path();
            if !theme_path.exists() {
                create_empty_theme_toml(&dir)?;
            }
        }

        Some(Commands::Wipe { yes, hard }) => {
            let conn = get_db(cli.memory, &dir)?;
            wipe_tasks(&conn, yes, hard)?
        }

        Some(Commands::Display { view }) => {
            let config = match read_config(&dir) {
                Ok(config) => config,
                Err(_) => {
                    create_sqlite_db(&dir)?;
                    println!("Successfully created the database to store your items in!");
                    read_config(&dir).unwrap()
                }
            };

            // This will handle the theme, making a default one if
            // One doesn't exist
            let theme_path = dir.theme_path();
            if !theme_path.exists() {
                create_empty_theme_toml(&dir)?;
            }

            // Now read it in
            let theme = read_theme(&dir)?;
            run_tui(cli.memory, dir, config, theme, view)?;
        }

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

        Some(Commands::Import { database }) => {
            let config = match read_config(&dir) {
                Ok(config) => config,
                Err(_) => {
                    create_sqlite_db(&dir)?;
                    println!("Could not find an existing database, creating a new one.");
                    read_config(&dir).unwrap()
                }
            };

            import_database(database, config)?;
            println!("Finished import tasks to current database.")
        }

        None => {
            let config = match read_config(&dir) {
                Ok(config) => config,
                Err(_) => {
                    create_sqlite_db(&dir)?;
                    println!("Successfully created the database to store your items in!");
                    read_config(&dir).unwrap()
                }
            };

            // This will handle the theme, making a default one if
            // One doesn't exist
            let theme_path = dir.theme_path();
            if !theme_path.exists() {
                create_empty_theme_toml(&dir)?;
            }

            // Now read it in
            let theme = read_theme(&dir)?;

            run_tui(cli.memory, dir, config, theme, Some(LayoutView::default()))?;
        }
    }

    Ok(())
}
