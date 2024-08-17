use std::collections::HashSet;
use std::io::{stdout, Stdout, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Print, PrintStyledContent, Stylize};
use crossterm::terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, ExecutableCommand, QueueableCommand};
use rusqlite::Connection;

use crate::backend::database::{get_all_db_contents, get_db};
use crate::backend::task::{Display, TaskList};

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
            display_filter: Display::All,
            urgency_sort_desc: true,
            tags_filter: None,
        }
    }
}

struct CursorInfo {
    cursor_x: u16,
    cursor_y: u16,
}

struct HighlightInfo {
    current_task: u64,
    highlight_x: u16,
    highlight_y: u16,
}

struct Graphics {
    vertical: String,
    horizontal: String,
    top_left: String,
    top_right: String,
    bottom_left: String,
    bottom_right: String,
}

impl Graphics {
    fn new() -> Self {
        Self {
            vertical: "─".to_string(),
            horizontal: "│".to_string(),
            top_left: "┌".to_string(),
            top_right: "┐".to_string(),
            bottom_left: "└".to_string(),
            bottom_right: "┘".to_string(),
        }
    }
}

struct Renderer {
    // DB connection
    conn: Connection,

    // Diplay attributes
    width: u16,
    height: u16,
    box_padding: u16,
    main_box_start: (u16, u16),
    detail_box_start: (u16, u16),
    graphics: Graphics,

    // Our stdout
    stdout: Stdout,

    // Information on tasks
    taskinfo: TaskInfo,

    // Information on where cursor is
    cursorinfo: CursorInfo,

    // Information on what task is currently highlighted
    highlightinfo: HighlightInfo,
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
            main_box_start: (box_padding, box_padding),
            detail_box_start: (width / 3, box_padding + 1),
            graphics: Graphics::new(),
            stdout,
            taskinfo: TaskInfo::new(),
            cursorinfo: CursorInfo {
                cursor_x: 0,
                cursor_y: 0,
            },
            highlightinfo: HighlightInfo {
                current_task: 0,
                highlight_x: 0,
                highlight_y: 0,
            },
        }
    }

    fn render(&mut self) -> Result<()> {
        self.stdout.queue(cursor::Hide)?;
        // Update task list
        self.pull_latest_tasklist()
            .context("Had an error pulling the latest tasklist")?;
        execute!(self.stdout, terminal::Clear(ClearType::All)).expect("Could not clear the screen");

        // Draw our main box
        self.draw_box(
            self.main_box_start.0,
            self.main_box_start.1,
            self.width - self.box_padding,
            self.height - self.box_padding,
        )?;

        // Draw detail box
        self.draw_box(
            self.detail_box_start.0,
            self.detail_box_start.1,
            self.width - self.box_padding - 1,
            self.height - self.box_padding - 1,
        )?;

        // Position cursor so we can draw out some helpful commands!
        self.stdout.queue(cursor::MoveTo(
            self.box_padding + 1,
            self.height - self.box_padding + 1,
        ))?;
        self.stdout
            .queue(Print("Actions: (a)dd    (u)pdate    (d)elete    e(x)it"))?;

        // Now render our task list items
        self.display_tasks()?;

        // Highlight current task
        self.set_highlight()?;

        // Dispaly details of current highlight
        self.display_details_of_current()?;

        // Finally, flush!
        self.stdout.flush()?;

        Ok(())
    }

    fn draw_box(&mut self, start_x: u16, start_y: u16, end_x: u16, end_y: u16) -> Result<()> {
        for i in start_x..=end_x {
            for j in start_y..=end_y {
                self.stdout.queue(cursor::MoveTo(i, j))?;

                if i == start_x && j == start_y {
                    self.stdout.queue(Print(&self.graphics.top_left))?;
                    continue;
                }
                if i == start_x && j == end_y {
                    self.stdout.queue(Print(&self.graphics.bottom_left))?;
                    continue;
                }
                if i == end_x && j == start_y {
                    self.stdout.queue(Print(&self.graphics.top_right))?;
                    continue;
                }
                if i == end_x && j == end_y {
                    self.stdout.queue(Print(&self.graphics.bottom_right))?;
                    continue;
                }

                if i == start_x || i == end_x {
                    self.stdout.queue(Print(&self.graphics.horizontal))?;
                }
                if j == start_y || j == end_y {
                    self.stdout.queue(Print(&self.graphics.vertical))?;
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
        self.cursorinfo.cursor_x = self.main_box_start.0 + 3;
        self.cursorinfo.cursor_y = self.main_box_start.1 + 1;

        for task in self.taskinfo.tasklist.tasks.iter() {
            self.stdout
                .queue(cursor::MoveTo(
                    self.cursorinfo.cursor_x,
                    self.cursorinfo.cursor_y,
                ))
                .context("Moving cursor during display_tasks()")?;

            let name = task.name.clone();
            let task_tags = task.tags.clone().unwrap_or(HashSet::new());
            let mut task_tags_vec: Vec<&String> = task_tags.iter().collect();
            task_tags_vec.sort_by(|a, b| b.cmp(a));

            // Print out tasks
            // First line - Title
            self.stdout
                .queue(PrintStyledContent(name.magenta().underlined()))?;
            // Second line - Status and tags
            self.stdout.queue(cursor::MoveTo(
                self.cursorinfo.cursor_x,
                self.cursorinfo.cursor_y + 1,
            ))?;
            let second_line = format!(
                "{} - {}",
                task.urgency.to_colored_string(),
                task.status.to_colored_string(),
            );
            self.stdout.queue(Print(second_line))?;

            self.stdout.queue(cursor::MoveTo(
                self.cursorinfo.cursor_x,
                self.cursorinfo.cursor_y + 2,
            ))?;
            let mut tags_string = String::from("Tags:");
            for tag in task_tags_vec {
                tags_string += &format!(" {}", tag.clone().blue());
            }
            // let second_line = format!("{}", tags_string);
            self.stdout.queue(Print(tags_string))?;

            // Third line - Date for when task was made
            self.stdout.queue(cursor::MoveTo(
                self.cursorinfo.cursor_x,
                self.cursorinfo.cursor_y + 3,
            ))?;
            let fourth_line = format!(
                "Made on: {}",
                task.date_added.date_naive().to_string().cyan()
            );
            self.stdout.queue(Print(fourth_line))?;

            self.cursorinfo.cursor_y += 5;
        }
        Ok(())
    }

    fn display_details_of_current(&mut self) -> Result<()> {
        // Get current task displayed
        let current_task = &self.taskinfo.tasklist.tasks[self.highlightinfo.current_task as usize];
        let name = current_task.name.clone();

        let task_tags = current_task.tags.clone().unwrap_or(HashSet::new());
        let mut task_tags_vec: Vec<&String> = task_tags.iter().collect();
        task_tags_vec.sort_by(|a, b| b.cmp(a));

        let column = self.detail_box_start.0 + 1;
        let mut row = self.detail_box_start.1 + 1;

        // Start printing
        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout
            .queue(Print(format!("Title: {}", name.magenta().underlined())))?;
        row += 1;

        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout.queue(Print(format!(
            "Made on: {}",
            current_task.date_added.date_naive().to_string().cyan()
        )))?;
        row += 1;

        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout.queue(Print(format!(
            "Status: {}",
            current_task.status.to_colored_string()
        )))?;
        match current_task.completed_on {
            Some(date) => {
                self.stdout.queue(Print(format!(
                    " - {}",
                    date.date_naive().to_string().green()
                )))?;
            }
            None => {}
        }
        row += 1;

        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout.queue(Print(format!(
            "Urgency: {}",
            current_task.urgency.to_colored_string()
        )))?;
        row += 1;

        self.stdout.queue(cursor::MoveTo(column, row))?;
        let mut tags_string = String::from("Tags:");
        for tag in task_tags_vec {
            tags_string += &format!(" {}", tag.clone().blue());
        }
        // let second_line = format!("{}", tags_string);
        self.stdout.queue(Print(tags_string))?;
        row += 2;

        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout.queue(Print(format!(
            "{} {}",
            "Latest Updates:".to_string().underlined(),
            current_task
                .latest
                .clone()
                .unwrap_or(String::from(""))
                .magenta()
        )))?;

        row += 2;
        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout
            .queue(PrintStyledContent("Description:".underlined()))?;

        row += 1;
        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout.queue(PrintStyledContent(
            current_task
                .description
                .clone()
                .unwrap_or(String::from(""))
                .grey(),
        ))?;

        Ok(())
    }

    fn set_highlight(&mut self) -> Result<()> {
        // First wipe all prior highlights
        for h in self.main_box_start.0 + 1..=self.height - self.box_padding - 1 {
            self.stdout
                .queue(cursor::MoveTo(self.main_box_start.0 + 1, h))?;
            self.stdout.queue(Print(" "))?;
        }

        // Set initial cursor position based on current task number
        self.highlightinfo.highlight_x = self.main_box_start.0 + 1;
        self.highlightinfo.highlight_y =
            self.box_padding + 1 + (5 * self.highlightinfo.current_task as u16);

        let highlight_length = 0..=3;
        for i in highlight_length {
            self.stdout.queue(cursor::MoveTo(
                self.highlightinfo.highlight_x,
                self.highlightinfo.highlight_y + i,
            ))?;
            self.stdout.queue(PrintStyledContent("█".cyan()))?;
        }

        // Set cursor back to top of item
        self.stdout.queue(cursor::MoveTo(
            self.highlightinfo.highlight_x,
            self.highlightinfo.highlight_y,
        ))?;

        Ok(())
    }
}

fn run(renderer: &mut Renderer) -> Result<bool> {
    // Need a way to display the data
    read_in_key(renderer)
}

fn read_in_key(renderer: &mut Renderer) -> Result<bool> {
    loop {
        if event::poll(Duration::from_millis(500))? {
            match event::read()? {
                Event::Key(event) => match event {
                    KeyEvent {
                        code: KeyCode::Char('x'),
                        modifiers: KeyModifiers::NONE,
                        kind: _,
                        state: _,
                    } => return Ok(false),
                    KeyEvent {
                        code: direction @ (KeyCode::Up | KeyCode::Down),
                        modifiers: KeyModifiers::NONE,
                        kind: _,
                        state: _,
                    } => handle_direction(renderer, direction)?,
                    _ => {}
                },
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

fn handle_direction(renderer: &mut Renderer, direction: KeyCode) -> Result<()> {
    match direction {
        KeyCode::Up => {
            if renderer.highlightinfo.current_task != 0 {
                renderer.highlightinfo.current_task -= 1;
            }
        }
        KeyCode::Down => {
            if renderer.highlightinfo.current_task as usize + 1 != renderer.taskinfo.tasklist.len()
            {
                renderer.highlightinfo.current_task += 1
            }
        }
        _ => panic!("We shouldn't be handling any other KeyCode here"),
    }
    renderer.render()?;
    renderer.stdout.flush()?;

    Ok(())
}
