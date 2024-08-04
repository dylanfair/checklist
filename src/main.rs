use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod subcommands;

use crate::subcommands::config::{read_config, Config};
use crate::subcommands::database::{add_to_db, create_sqlite_db, get_all_db_contents, get_db};
use crate::subcommands::task::{Task, Urgency};
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

    /// Wipe out all tasks
    Wipe {
        /// Bypass confirmation check
        #[arg(short)]
        yes: bool,
    },

    /// Adds a task to your checklist
    Add {
        /// Name of the task
        #[arg(short, long)]
        name: String,

        /// Description of the task
        #[arg(short, long)]
        description: Option<String>,

        /// Latest updates on the task
        #[arg(short, long)]
        latest: Option<String>,

        /// Urgency of the task
        #[arg(short, long, value_enum)]
        urgency: Option<Urgency>,
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
        }) => {
            println!("Create task");
            let new_task = Task::new(name, description, latest, urgency);
            println!("{:?}", new_task);

            let conn = get_db(cli.memory, cli.test)?;
            add_to_db(&conn, new_task)?;
            println!("New task added successfully");
        }

        Some(Commands::List { completed }) => {
            let conn = get_db(cli.memory, cli.test)?;
            let tasks = get_all_db_contents(&conn).unwrap();
            println!("Found {:?} tasks", tasks.len());
            println!("id | name | description | latest | urgency | status | completed_on");
            for task in tasks {
                let print_fmt = format!(
                    "{:?} | {:?} | {:?} | {:?} | {:?} | {:?} | {:?} ",
                    task.get_id(),
                    task.name,
                    task.description,
                    task.latest,
                    task.urgency,
                    task.status,
                    task.completed_on
                );
                println!("{}", print_fmt);
            }
        }

        Some(Commands::Wipe { yes }) => {
            let conn = get_db(cli.memory, cli.test)?;
            wipe_tasks(&conn, yes)?
        }

        None => {}
    }

    Ok(())
}
