use std::collections::HashSet;
use std::io::{stdout, Cursor, Stdout, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Print, PrintStyledContent, Stylize};
use crossterm::terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, ExecutableCommand, QueueableCommand};
use rusqlite::Connection;

use crate::backend::database::{get_all_db_contents, get_db};
use crate::backend::task::{Display, TaskList, Urgency};

struct CleanUp;

impl Drop for CleanUp {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Could not disable raw mode");
        execute!(stdout(), terminal::Clear(ClearType::All)).expect("Could not clear the screen");
        execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
    }
}

pub fn run_ui(memory: bool, testing: bool) -> Result<()> {
    let _clean_up = CleanUp;
    let conn = get_db(memory, testing).context("Errored out making a database connection")?;
    terminal::enable_raw_mode().expect("Could not turn on raw mode");

    let mut renderer = Renderer::new(3, conn);
    renderer.stdout.execute(EnterAlternateScreen)?;
    renderer.render()?;

    while run(&mut renderer)? {}

    Ok(())
}

struct TaskInfo {
    tasklist: TaskList,
    display_filter: Display,
    urgency_sort_desc: bool,
    tags_filter: Option<Vec<String>>,
}

impl TaskInfo {
    fn new() -> Self {
        Self {
            tasklist: TaskList::new(),
            display_filter: Display::NotCompleted,
            urgency_sort_desc: true,
            tags_filter: None,
        }
    }
}

struct CursorInfo {
    cursor_x: u16,
    cursor_y: u16,
}

struct Renderer {
    conn: Connection,
    width: u16,
    height: u16,
    box_padding: u16,
    stdout: Stdout,
    taskinfo: TaskInfo,
    cursorinfo: CursorInfo,
}

impl Renderer {
    fn new(box_padding: u16, conn: Connection) -> Self {
        let (width, height) = terminal::size().unwrap();
        let stdout = stdout();
        Self {
            conn,
            width,
            height,
            box_padding,
            stdout,
            taskinfo: TaskInfo::new(),
            cursorinfo: CursorInfo {
                cursor_x: 0,
                cursor_y: 0,
            },
        }
    }

    fn render(&mut self) -> Result<()> {
        // Update task list
        self.pull_latest_tasklist()
            .context("Had an error pulling the latest tasklist")?;
        execute!(self.stdout, terminal::Clear(ClearType::All)).expect("Could not clear the screen");

        // Draw our box
        self.draw_box()?;

        // Position cursor so we can draw out some helpful commands!
        self.stdout.queue(cursor::MoveTo(
            self.box_padding + 1,
            self.height - self.box_padding + 1,
        ))?;
        self.stdout
            .queue(Print("Actions: (a)dd    (u)pdate    (d)elete    e(x)it"))?;

        // Now render our task list items
        self.display_tasks()?;

        // Finally, flush!
        self.stdout.flush()?;

        Ok(())
    }

    fn draw_box(&mut self) -> Result<()> {
        let vertical_char = "─";
        let horizontal_char = "│";
        let top_left = "┌";
        let top_right = "┐";
        let bottom_left = "└";
        let bottom_right = "┘";

        for i in self.box_padding..=self.width - self.box_padding {
            for j in self.box_padding..=self.height - self.box_padding {
                self.stdout.queue(cursor::MoveTo(i, j))?;

                if i == self.box_padding && j == self.box_padding {
                    self.stdout.queue(Print(top_left))?;
                    continue;
                }
                if i == self.box_padding && j == self.height - self.box_padding {
                    self.stdout.queue(Print(bottom_left))?;
                    continue;
                }
                if i == self.width - self.box_padding && j == self.box_padding {
                    self.stdout.queue(Print(top_right))?;
                    continue;
                }
                if i == self.width - self.box_padding && j == self.height - self.box_padding {
                    self.stdout.queue(Print(bottom_right))?;
                    continue;
                }

                if i == self.box_padding || i == self.width - self.box_padding {
                    self.stdout.queue(Print(horizontal_char))?;
                }
                if j == self.box_padding || j == self.height - self.box_padding {
                    self.stdout.queue(Print(vertical_char))?;
                }
            }
        }
        Ok(())
    }

    fn pull_latest_tasklist(&mut self) -> Result<()> {
        // Get data
        let task_list = get_all_db_contents(&self.conn).unwrap();
        self.taskinfo.tasklist = task_list;

        // Filter tasks
        self.taskinfo.tasklist.filter_tasks(
            Some(self.taskinfo.display_filter),
            self.taskinfo.tags_filter.clone(),
        );

        // Order tasks here
        self.taskinfo
            .tasklist
            .sort_by_urgency(self.taskinfo.urgency_sort_desc);

        Ok(())
    }

    pub fn display_tasks(&mut self) -> Result<()> {
        self.cursorinfo.cursor_x = self.box_padding + 1;
        self.cursorinfo.cursor_y = self.box_padding + 1;

        for task in self.taskinfo.tasklist.tasks.iter() {
            self.stdout
                .queue(cursor::MoveTo(
                    self.cursorinfo.cursor_x,
                    self.cursorinfo.cursor_y,
                ))
                .context("Moving cursor during display_tasks()")?;

            let name = task.name.clone();
            let description = task.description.clone().unwrap_or(String::from("None"));
            let latest = task.latest.clone().unwrap_or(String::from("None"));
            let task_tags = task.tags.clone().unwrap_or(HashSet::new());

            // Print out tasks
            // First line - Urgency and Title
            let first_line = format!(
                "{} - {}",
                task.urgency.to_colored_string(),
                name.underlined()
            );
            self.stdout.queue(Print(first_line))?;

            // Second line - Status and tags
            self.stdout.queue(cursor::MoveTo(
                self.cursorinfo.cursor_x,
                self.cursorinfo.cursor_y + 1,
            ))?;
            let mut tags_string = String::from("Tags:");
            for tag in task_tags {
                tags_string += &format!(" {}", tag.blue());
            }
            let second_line = format!("{} | {}", task.status.to_colored_string(), tags_string);
            self.stdout.queue(Print(second_line))?;

            // Third line - Date for when task was made
            self.stdout.queue(cursor::MoveTo(
                self.cursorinfo.cursor_x,
                self.cursorinfo.cursor_y + 2,
            ))?;
            let third_line = format!(
                "Made on: {}",
                task.date_added.date_naive().to_string().cyan()
            );
            self.stdout.queue(Print(third_line))?;
            //print!(" Tags:");
            //for tag in task_tags {
            //    print!(" {}", tag.blue());
            //}
            //print!("\n");
            //print!(
            //    "   {} | {}",
            //    task.urgency.to_colored_string(),
            //    task.status.to_colored_string()
            //);
            //match task.completed_on {
            //    Some(date) => {
            //        print!(" - {}", date.date_naive().to_string().green())
            //    }
            //    None => {}
            //}
            //print!("\n");
            //println!(
            //    "   Date Added: {}",
            //    task.date_added.date_naive().to_string().cyan()
            //);
            //println!("  Description: {}", description.blue());
            //println!("  Latest Update: {}", latest.blue());

            self.cursorinfo.cursor_y += 4;
        }
        Ok(())
    }
}

fn run(renderer: &mut Renderer) -> Result<bool> {
    // Need a way to display the data
    process_keypress(renderer)
}

fn process_keypress(renderer: &mut Renderer) -> Result<bool> {
    match read_in_key(renderer)? {
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

fn read_in_key(renderer: &mut Renderer) -> Result<KeyEvent> {
    loop {
        if event::poll(Duration::from_millis(33))? {
            match event::read()? {
                Event::Key(event) => return Ok(event),
                Event::Resize(nw, nh) => {
                    renderer.width = nw;
                    renderer.height = nh;
                    renderer.render()?;
                }
                _ => {}
            }
        }
    }
}
