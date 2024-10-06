use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod backend;
mod display;

use backend::config::{get_config_dir, read_config, set_new_path};
use backend::database::{create_sqlite_db, get_db};
use backend::wipe::wipe_tasks;

use display::theme::{get_toml_file, read_theme};
use display::tui::{run_tui, LayoutView};
use display::ui::run_ui;

use crate::display::theme::create_empty_theme_toml;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Will run checklist off of a memory SQLite database.
    /// As a result, no data will be kept on program exit.
    #[arg(short, long)]
    memory: bool,

    /// Will run checklist off a test SQLite database.
    /// This will keep data in a test.checklist.sqlite file.
    #[arg(short, long)]
    test: bool,

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
        #[arg(short, long)]
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
        /// For testing, switches between ratatui or my hand-rolled interface
        #[arg(long)]
        old: bool,

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { set }) => {
            if let Some(valid_path) = set {
                set_new_path(valid_path, cli.test)?;
            } else {
                // Probably need to decouple, but this will make the config
                // file and the sqlite db
                create_sqlite_db(cli.test)?;
                println!("Successfully created the database to store your items in!");
            }

            // This will handle the theme, making a default one if
            // One doesn't exist
            let toml_file = get_toml_file()?;

            if !toml_file.exists() {
                create_empty_theme_toml()?;
            }
        }

        Some(Commands::Wipe { yes, hard }) => {
            let conn = get_db(cli.memory, cli.test)?;
            wipe_tasks(&conn, yes, hard)?
        }

        Some(Commands::Display { old, view }) => {
            let config = match read_config(cli.test) {
                Ok(config) => config,
                Err(_) => {
                    create_sqlite_db(cli.test)?;
                    println!("Successfully created the database to store your items in!");
                    read_config(cli.test).unwrap()
                }
            };

            // This will handle the theme, making a default one if
            // One doesn't exist
            let toml_file = get_toml_file()?;
            if !toml_file.exists() {
                create_empty_theme_toml()?;
            }

            // Now read it in
            let theme = read_theme()?;
            if old {
                run_ui(cli.memory, cli.test)?;
            } else {
                run_tui(cli.memory, cli.test, config, theme, view)?;
            }
        }

        Some(Commands::Where { db, config, theme }) => match get_config_dir() {
            Ok(dir) => {
                if !db & !config & !theme {
                    println!("{}", dir.to_str().unwrap());
                }
                if db {
                    let db_path = if cli.test {
                        dir.join(String::from("test.checklist.sqlite"))
                    } else {
                        dir.join(String::from("checklist.sqlite"))
                    };
                    if db_path.exists() {
                        println!("{}", db_path.to_str().unwrap());
                    } else {
                        println!("Could not find a SQLite database file.")
                    }
                }
                if config {
                    let config_path = if cli.test {
                        dir.join(String::from("test.config.json"))
                    } else {
                        dir.join(String::from("config.json"))
                    };
                    if config_path.exists() {
                        println!("{}", config_path.to_str().unwrap());
                    } else {
                        println!("Could not find a config file.")
                    }
                }
                if theme {
                    let theme_path = dir.join(String::from("theme.toml"));
                    if theme_path.exists() {
                        println!("{}", theme_path.to_str().unwrap());
                    } else {
                        println!("Could not find a theme file.")
                    }
                }
            }
            Err(_) => {
                println!("Could not find the folder that should hold checklist files");
                println!("Try getting started with 'checklist init' or 'checklist'!");
            }
        },

        None => {
            let config = match read_config(cli.test) {
                Ok(config) => config,
                Err(_) => {
                    create_sqlite_db(cli.test)?;
                    println!("Successfully created the database to store your items in!");
                    read_config(cli.test).unwrap()
                }
            };

            // This will handle the theme, making a default one if
            // One doesn't exist
            let toml_file = get_toml_file()?;
            if !toml_file.exists() {
                create_empty_theme_toml()?;
            }

            // Now read it in
            let theme = read_theme()?;

            run_tui(
                cli.memory,
                cli.test,
                config,
                theme,
                Some(LayoutView::default()),
            )?;
        }
    }

    Ok(())
}
