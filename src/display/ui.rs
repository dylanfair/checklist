use std::io::stdout;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, ClearType};

use crate::backend::database::{get_all_db_contents, get_db};
use crate::backend::task::Display;

struct CleanUp;

impl Drop for CleanUp {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Could not disable raw mode");
        execute!(stdout(), terminal::Clear(ClearType::All)).expect("Could not clear the screen");
    }
}

pub fn run_ui(memory: bool, testing: bool) -> Result<()> {
    let _clean_up = CleanUp;
    terminal::enable_raw_mode().expect("Could not turn on raw mode");
    execute!(stdout(), terminal::Clear(ClearType::All)).expect("Could not clear the screen");

    // Get data
    let conn = get_db(memory, testing).context("Errored out making a database connection")?;
    let mut task_list = get_all_db_contents(&conn).unwrap();

    // Filter tasks
    task_list.filter_tasks(Some(Display::NotCompleted), None);

    // Order tasks here
    task_list.sort_by_urgency(true);

    while run()? {}

    Ok(())
}

fn run() -> Result<bool> {
    // Need a way to display the data
    process_keypress()
}

fn process_keypress() -> Result<bool> {
    match read_in_key()? {
        KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: _,
            state: _,
        } => return Ok(false),
        _ => {}
    }
    Ok(true)
}

fn read_in_key() -> Result<KeyEvent> {
    loop {
        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(event) = event::read()? {
                return Ok(event);
            }
        }
    }
}
