use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod backend;
mod display;

use backend::config::set_new_path;
use backend::database::{add_to_db, create_sqlite_db, get_all_db_contents, get_db};
use backend::task::{Display, Status, Task, Urgency};
use backend::wipe::wipe_tasks;

// use display::list_example::list_example;
use display::tui::run_tui;
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

    /// List out tasks
    List {
        /// Optional: Different display options
        /// By default will show tasks not-completed
        #[arg(short, long, value_enum)]
        display: Option<Display>,

        /// Optional: Filter for tasks with the following tags
        /// Can supply multiple
        #[arg(short, long, num_args=1..)]
        tag: Option<Vec<String>>,
    },

    /// Wipe out all tasks in the sqlite database
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
        display: bool,
    },
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

        Some(Commands::List { display, tag }) => {
            let conn = get_db(cli.memory, cli.test)?;
            let mut task_list = get_all_db_contents(&conn).unwrap();

            // Filter tasks
            task_list.filter_tasks(display, tag);

            // Order tasks here
            task_list.sort_by_urgency(true);

            // Print out tasks
            task_list.display_tasks();
        }

        Some(Commands::Wipe { yes, hard }) => {
            let conn = get_db(cli.memory, cli.test)?;
            wipe_tasks(&conn, yes, hard)?
        }

        Some(Commands::Display { display }) => {
            if display {
                run_ui(cli.memory, cli.test)?;
            } else {
                run_tui(cli.memory, cli.test)?;
            }
        }

        None => {}
    }

    Ok(())
}
