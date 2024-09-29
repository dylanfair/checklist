use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod backend;
mod display;

use backend::config::{read_config, set_new_path};
use backend::database::{add_to_db, create_sqlite_db, get_db};
use backend::task::{Status, Task, Urgency};
use backend::wipe::wipe_tasks;

use display::tui::{run_tui, LayoutView};
use display::ui::run_ui;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional for testing using an in memory sqlite db
    #[arg(short, long)]
    memory: bool,

    /// Optional for testing using a test sqlite db
    #[arg(short, long)]
    test: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initializes checklist, creating a sqlite database
    /// that the program will automatically use
    /// and a config json file
    Init {
        /// Optional argument that will set a given
        /// sqlite database as the new default
        #[arg(short, long)]
        set: Option<PathBuf>,
    },

    /// Wipe out all tasks
    Wipe {
        /// Bypass confirmation check
        #[arg(short)]
        yes: bool,

        /// Pass in to drop the table entirely
        #[arg(long)]
        hard: bool,
    },

    /// Adds a task to your checklist
    Add {
        /// Name of the task
        #[arg(short, long)]
        name: String,

        /// Optional: Description of the task
        #[arg(short, long)]
        description: Option<String>,

        /// Optional: Latest updates on the task
        #[arg(short, long)]
        latest: Option<String>,

        /// Optional: Urgency of the task
        #[arg(short, long, value_enum)]
        urgency: Option<Urgency>,

        /// Optional: Status of the task
        #[arg(short, long, value_enum)]
        status: Option<Status>,

        /// Optional: Tags to give the task
        #[arg(short, long, num_args = 1..)]
        tag: Option<Vec<String>>,
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

    /// Tells you where the sqlite db that stores your task are
    Where {},
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { set }) => {
            if let Some(valid_path) = set {
                set_new_path(valid_path, cli.test)?;
            } else {
                create_sqlite_db(cli.test)?;
                println!("Successfully created the database to store your items in!");
            }
        }

        Some(Commands::Add {
            name,
            description,
            latest,
            urgency,
            status,
            tag,
        }) => {
            println!("Create task");
            let mut hashset = None;
            if let Some(t) = tag {
                hashset = Some(HashSet::from_iter(t));
            }
            let new_task = Task::new(name, description, latest, urgency, status, hashset);
            println!("{:?}", new_task);

            let conn = get_db(cli.memory, cli.test)?;
            add_to_db(&conn, &new_task)?;
            println!("New task added successfully");
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
            if old {
                run_ui(cli.memory, cli.test)?;
            } else {
                run_tui(cli.memory, cli.test, config, view)?;
            }
        }

        Some(Commands::Where {}) => {
            match read_config(cli.test) {
                Ok(config) => {
                    println!("Your tasks are stored in the following database:");
                    println!("{}", config.db_path.to_str().unwrap());
                }
                Err(_) => {
                    println!("Could not find a current configruation file.");
                    println!("Try getting started with 'checklist init' or 'checklist'!");
                }
            };
        }

        None => {
            let config = match read_config(cli.test) {
                Ok(config) => config,
                Err(_) => {
                    create_sqlite_db(cli.test)?;
                    println!("Successfully created the database to store your items in!");
                    read_config(cli.test).unwrap()
                }
            };
            run_tui(cli.memory, cli.test, config, Some(LayoutView::default()))?;
        }
    }

    Ok(())
}
