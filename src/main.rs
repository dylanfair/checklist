use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod subcommands;

use crate::subcommands::config::{read_config, Config};
use crate::subcommands::database::{add_to_db, create_sqlite_db, get_all_db_contents, get_db};
use crate::subcommands::task::{Status, Task, Urgency};
use crate::subcommands::wipe::wipe_tasks;

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
        /// Include completed
        #[arg(short, long)]
        completed: bool,
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

        /// Optional: Status of the task
        #[arg(short, long, num_args = 1..)]
        tag: Option<Vec<String>>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { set }) => {
            if let Some(valid_path) = set {
                match read_config(cli.test) {
                    Ok(mut config) => {
                        config.db_path = valid_path.clone();
                        config.save(cli.test)?;
                        println!("Updated db path to {:?}", valid_path);
                    }
                    Err(_) => {
                        let config = Config::new(valid_path.clone());
                        config.save(cli.test)?;
                        println!("Set db path to {:?}", valid_path);
                    }
                }
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
            let new_task = Task::new(name, description, latest, urgency, status, tag);
            println!("{:?}", new_task);

            let conn = get_db(cli.memory, cli.test)?;
            add_to_db(&conn, new_task)?;
            println!("New task added successfully");
        }

        Some(Commands::List { completed }) => {
            let conn = get_db(cli.memory, cli.test)?;
            let mut task_list = get_all_db_contents(&conn).unwrap();
            println!("Found {} tasks\n", task_list.len());

            // Order tasks here
            task_list.sort_by_urgency(true);

            // Print out tasks
            task_list.display_tasks();
        }

        Some(Commands::Wipe { yes, hard }) => {
            let conn = get_db(cli.memory, cli.test)?;
            wipe_tasks(&conn, yes, hard)?
        }

        None => {}
    }

    Ok(())
}
