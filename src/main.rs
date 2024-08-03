use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod subcommands;

use crate::subcommands::{config::save_db_path, database::create_sqlite_db};

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
    /// Creates a sqlite db to store items in
    Setup {
        /// Path to make sqlite db at.
        /// If not an update, a folder can be given where a
        /// 'checklist.sqlite' will be made.
        /// This can also be left made blank. Where in the sqlite db will
        /// be made in your respective App Folder
        /// If an update, the path must include the new sqlite database
        /// to use.
        #[arg(short, long)]
        path: Option<PathBuf>,

        #[arg(short, long)]
        update: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Setup { path, update }) => {
            if *update {
                match path {
                    Some(valid_path) => {
                        println!("Updated db path to {:?}", valid_path);
                    }
                    None => {
                        println!("A path needs to be given if updating")
                    }
                }
            } else {
                create_sqlite_db(path.clone(), cli.memory)?;
                println!("Successfully created the database to store your items in!");
            }
        }
        None => {}
    }

    Ok(())
}
