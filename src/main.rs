use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod subcommands;

use crate::subcommands::config::{read_config, Config};
use crate::subcommands::database::create_sqlite_db;
use crate::subcommands::task::{Task, Urgency};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional for testing using an in memory sqlite db
    #[arg(short, long)]
    memory: bool,

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

    /// Adds a task to your checklist
    Add {
        /// Name of the task
        #[arg(short, long)]
        name: String,

        /// Description of the task
        #[arg(short, long)]
        description: Option<String>,

        /// Urgency of the task
        #[arg(short, long, value_enum, value_parser)]
        urgency: Option<Urgency>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Init { set }) => {
            if let Some(valid_path) = set {
                match read_config() {
                    Ok(mut config) => {
                        config.db_path = valid_path.clone();
                        config.save()?;
                        println!("Updated db path to {:?}", valid_path);
                    }
                    Err(_) => {
                        let config = Config::new(valid_path.clone());
                        config.save()?;
                        println!("Set db path to {:?}", valid_path);
                    }
                }
            } else {
                create_sqlite_db(false)?;
                println!("Successfully created the database to store your items in!");
            }
        }
        Some(Commands::Add {
            name,
            description,
            urgency,
        }) => {
            println!("Name");
            let new_task = Task::new(name.to_string(), description.clone(), urgency.clone());
        }
        None => {}
    }

    Ok(())
}
