use std::collections::HashSet;
use std::io::{stdout, Stdout, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Print, PrintStyledContent, SetForegroundColor, Stylize};
use crossterm::terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, ExecutableCommand, QueueableCommand};
use rusqlite::Connection;

use crate::backend::database::{get_all_db_contents, get_db};
use crate::backend::task::{Display, Task, TaskList};

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

    let mut renderer = Renderer::new(3, 5, conn);
    renderer.stdout.queue(cursor::Hide)?;
    renderer.stdout.execute(EnterAlternateScreen)?;
    renderer.pull_latest_tasklist()?;
    renderer.render()?;

    while run(&mut renderer)? {}

    Ok(())
}

struct TaskInfo {
    total_tasklist: TaskList,
    display_filter: Display,
    urgency_sort_desc: bool,
    tags_filter: String,
    current_task: u64,
    current_task_details_len: u64,
    display_tasklist: TaskList,
}

impl TaskInfo {
    fn new() -> Self {
        Self {
            total_tasklist: TaskList::new(),
            display_filter: Display::All,
            urgency_sort_desc: true,
            tags_filter: String::new(),
            current_task: 0,
            current_task_details_len: 0,
            display_tasklist: TaskList::new(),
        }
    }
}

struct CursorInfo {
    cursor_x: u16,
    cursor_y: u16,
}

struct HighlightInfo {
    highlight_place: u64,
    highlight_x: u16,
    highlight_y: u16,
}

struct TaskWindow {
    window_start: i64,
    window_end: i64,
    tasks_that_can_fit: u16,
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
    task_height: u16,

    // Our stdout
    stdout: Stdout,

    // Information on tasks
    taskinfo: TaskInfo,

    // Window of tasks we want to display
    taskwindow: TaskWindow,

    // Information on where cursor is
    cursorinfo: CursorInfo,

    // Information on what task is currently highlighted
    highlightinfo: HighlightInfo,
}

impl Renderer {
    fn new(box_padding: u16, task_height: u16, conn: Connection) -> Self {
        let (width, height) = terminal::size().unwrap();
        let stdout = stdout();
        let main_window_height = (height - (box_padding * 2)) / task_height;
        Self {
            conn,
            width,
            height,
            box_padding,
            task_height,
            main_box_start: (box_padding, box_padding),
            detail_box_start: (width / 3, box_padding + 1),
            graphics: Graphics::new(),
            stdout,
            taskinfo: TaskInfo::new(),
            taskwindow: TaskWindow {
                window_start: 0,
                window_end: main_window_height as i64 - 1,
                tasks_that_can_fit: main_window_height - 1,
            },
            cursorinfo: CursorInfo {
                cursor_x: 0,
                cursor_y: 0,
            },
            highlightinfo: HighlightInfo {
                highlight_place: 0,
                highlight_x: 0,
                highlight_y: 0,
            },
        }
    }

    fn render(&mut self) -> Result<()> {
        // Update task list
        //self.pull_latest_tasklist()?;
        // Set our task window
        self.update_task_window();

        execute!(self.stdout, terminal::Clear(ClearType::All)).expect("Could not clear the screen");

        // Draw our main box
        self.draw_box(
            self.main_box_start.0,
            self.main_box_start.1,
            self.width - self.box_padding,
            self.height - self.box_padding,
        )?;

        self.stdout.queue(cursor::MoveTo(
            self.main_box_start.0,
            self.main_box_start.1 - 1,
        ))?;
        self.stdout.queue(PrintStyledContent(
            "Welcome to your Checklist!".underlined().bold(),
        ))?;

        // Position cursor so we can draw out some helpful commands!
        self.stdout.queue(cursor::MoveTo(
            self.box_padding + 1,
            self.height - self.box_padding + 1,
        ))?;
        self.stdout
            .queue(Print("Actions: (a)dd    (u)pdate    (d)elete    e(x)it"))?;

        // Now render our task list items
        self.display_tasks()?;

        // Display details of current highlight
        if self.taskinfo.total_tasklist.len() == 0 {
            let middle_message = String::from("Add some tasks!");
            self.stdout
                .queue(cursor::MoveTo(
                    (self.width / 2) - middle_message.chars().count() as u16,
                    self.height / 2,
                ))?
                .queue(Print(middle_message))?;
        } else {
            // Highlight current task
            self.set_highlight()?;
            //
            self.render_task_scroll_bar()?;
            // Draw detail box
            self.draw_box(
                self.detail_box_start.0,
                self.detail_box_start.1,
                self.width - self.box_padding - 1,
                self.height - self.box_padding - 1,
            )?;
            // Display details in box
            self.display_details_of_current()?;
        }

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
        self.taskinfo.total_tasklist = task_list;

        // Filter tasks
        self.taskinfo.total_tasklist.filter_tasks(
            Some(self.taskinfo.display_filter),
            self.taskinfo.tags_filter.clone(),
        );

        // Order tasks here
        self.taskinfo
            .total_tasklist
            .sort_by_urgency(self.taskinfo.urgency_sort_desc);

        Ok(())
    }

    fn update_task_window(&mut self) {
        // let current_tasks_in_window: &[Task];
        let current_tasks_in_window: &[Task] =
            if self.taskinfo.total_tasklist.len() <= self.taskwindow.tasks_that_can_fit as usize {
                &self.taskinfo.total_tasklist.tasks[0..]
            } else {
                &self.taskinfo.total_tasklist.tasks
                    [self.taskwindow.window_start as usize..=self.taskwindow.window_end as usize]
            };

        self.taskinfo.display_tasklist = TaskList::from(current_tasks_in_window.to_vec());
    }

    pub fn display_tasks(&mut self) -> Result<()> {
        self.cursorinfo.cursor_x = self.main_box_start.0 + 3;
        self.cursorinfo.cursor_y = self.main_box_start.1 + 1;

        for task in self.taskinfo.display_tasklist.tasks.iter() {
            self.stdout
                .queue(cursor::MoveTo(
                    self.cursorinfo.cursor_x,
                    self.cursorinfo.cursor_y,
                ))
                .context("Moving cursor during display_tasks()")?;

            let name = task.name.clone();
            let task_tags = task.tags.clone().unwrap_or(HashSet::new());
            let mut task_tags_vec: Vec<&String> = task_tags.iter().collect();
            task_tags_vec.sort();
            //task_tags_vec.sort_by(|a, b| a.cmp(b));

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

            self.cursorinfo.cursor_y += self.task_height;
        }
        Ok(())
    }

    fn display_details_of_current(&mut self) -> Result<()> {
        // Get width of details box
        // let width = self.width - self.box_padding - self.detail_box_start.0;
        let width = (self.width - self.box_padding - 1) - (self.detail_box_start.0);

        // Get current task displayed
        let current_task =
            &self.taskinfo.total_tasklist.tasks[self.taskinfo.current_task as usize].clone();
        let name = current_task.name.clone();

        let task_tags = current_task.tags.clone().unwrap_or_default();
        let mut task_tags_vec: Vec<&String> = task_tags.iter().collect();
        task_tags_vec.sort();

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
        if let Some(date) = current_task.completed_on {
            self.stdout.queue(Print(format!(
                " - {}",
                date.date_naive().to_string().green()
            )))?;
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
        self.stdout
            .queue(PrintStyledContent("Latest Updates:".underlined()))?;

        row += 1;
        let latest_updates = current_task.latest.clone().unwrap_or(String::from(""));
        self.wrap_lines(latest_updates, column, row, width, Color::Magenta)?;

        row = cursor::position()?.1; // reorient since could be anywhere after line wraaps
        row += 2;
        self.stdout.queue(cursor::MoveTo(column, row))?;
        self.stdout
            .queue(PrintStyledContent("Description:".underlined()))?;

        row += 1;
        let description = current_task.description.clone().unwrap_or(String::from(""));
        self.wrap_lines(description, column, row, width, Color::Grey)?;

        self.taskinfo.current_task_details_len = cursor::position()?.1 as u64;

        //row = cursor::position()?.1; // reorient since could be anywhere after line wraaps
        //row += 2;
        //let displayable_task_length = self.taskinfo.display_tasklist.len();
        //let scrollable_height = self.height - (self.box_padding * 2) - 2;
        //
        //self.stdout.queue(cursor::MoveTo(column, row))?;
        //let variables = format!(
        //    "display_tasklist_length: {} - scrollable_height: {} - total_tasklist_len: {}",
        //    displayable_task_length,
        //    self.detail_box_start.1 + scrollable_height,
        //    self.taskinfo.total_tasklist.len()
        //);
        //self.stdout.queue(Print(variables))?;
        //
        //row += 1;
        //self.stdout.queue(cursor::MoveTo(column, row))?;
        //let scrollable_height = self.height - (self.box_padding * 2) - 2;
        //let display_ratio = (self.taskinfo.display_tasklist.len() as f64)
        //    / self.taskinfo.total_tasklist.len() as f64;
        ////let scrollbar_ratio = scrollable_height as f64 / amount_of_tasks as f64;
        ////let reverse_ratio = amount_of_tasks as f64 / scrollable_height as f64;
        ////let fix = scrollbar_ratio * self.taskwindow.tasks_that_can_fit as f64;
        //let extra_space = scrollable_height as i64
        //    - (self.taskinfo.display_tasklist.len() as i64 * self.task_height as i64)
        //    + 2;
        //let ratio_w_height = scrollable_height as f64 * display_ratio;
        //let scrollbar_ratiod = (scrollable_height as f64 - extra_space as f64) * display_ratio;
        //let new_variables = format!(
        //    "display_ratio: {} - ratio_w_height: {}",
        //    display_ratio, ratio_w_height
        //);
        //self.stdout.queue(Print(new_variables))?;
        //
        //row += 1;
        //
        //let extra_space = scrollable_height as i64
        //    - (self.taskinfo.display_tasklist.len() as i64 * self.task_height as i64)
        //    + 2;
        //self.stdout.queue(cursor::MoveTo(column, row))?;
        //self.stdout.queue(Print(format!(
        //    "ratio_w_height as i64: {} - extra_space: {}",
        //    ratio_w_height as i64, extra_space
        //)))?;
        //
        //row += 1;
        //let bar_start = self.detail_box_start.1 as i64 + self.taskwindow.window_start as i64;
        ////let bar_end = bar_start + (scrollbar_ratiod as i64) + extra_space;
        //let bar_end = bar_start + scrollbar_ratiod as i64 + extra_space;
        //self.stdout.queue(cursor::MoveTo(column, row))?;
        //self.stdout.queue(Print(format!(
        //    "bar_start: {} - bar_end: {}",
        //    bar_start, bar_end
        //)))?;
        //
        //let scrollbar_height_to_tasks =
        //    scrollable_height as f64 / self.taskinfo.display_tasklist.len() as f64;
        //row += 1;
        //self.stdout.queue(cursor::MoveTo(column, row))?;
        //self.stdout.queue(Print(format!(
        //    "scrollbar_height_to_tasks: {}",
        //    scrollbar_height_to_tasks
        //)))?;
        //
        //let hidden_tasks =
        //    self.taskinfo.total_tasklist.len() - self.taskinfo.display_tasklist.len();
        //let scrollbar = scrollable_height - (hidden_tasks as u16 * self.task_height as u16);
        //row += 1;
        //self.stdout.queue(cursor::MoveTo(column, row))?;
        //self.stdout.queue(Print(format!(
        //    "scrollbar: {} - hidden_tasks: {}",
        //    scrollbar, hidden_tasks
        //)))?;

        Ok(())
    }

    fn wrap_lines(
        &mut self,
        lines: String,
        start_x: u16,
        mut start_y: u16,
        width: u16,
        text_color: Color,
    ) -> Result<()> {
        self.stdout.queue(cursor::MoveTo(start_x, start_y))?;
        self.stdout.queue(SetForegroundColor(text_color))?;
        let number_of_breaks = lines.chars().count() / (width as usize - 3); // giving some
                                                                             // space on the
                                                                             // side
        if number_of_breaks == 0 {
            self.stdout.queue(Print(lines))?;
        } else {
            let words = lines.split_whitespace();
            let mut current_line_usage = width as i32; // in case we go negative
            for word in words {
                if word.chars().count() >= current_line_usage as usize - 3 {
                    start_y += 1;
                    self.stdout.queue(cursor::MoveTo(start_x, start_y))?;
                    current_line_usage = width as i32;
                }
                self.stdout.queue(Print(format!("{} ", word)))?;
                current_line_usage -= word.chars().count() as i32 + 1;
            }
        }
        self.stdout.queue(SetForegroundColor(Color::Reset))?;
        Ok(())
    }

    fn set_highlight(&mut self) -> Result<()> {
        // First wipe all prior highlights
        for h in self.main_box_start.0 + 1..=self.height - self.box_padding - 1 {
            self.stdout
                .queue(cursor::MoveTo(self.main_box_start.0 + 1, h))?;
            self.stdout.queue(Print(" "))?;
        }

        // Set initial cursor position based on whereh highter should be
        self.highlightinfo.highlight_x = self.main_box_start.0 + 1;
        self.highlightinfo.highlight_y =
            self.box_padding + 1 + (self.task_height * self.highlightinfo.highlight_place as u16);

        let highlight_length = 0..=self.task_height - 2;
        for i in highlight_length {
            self.stdout.queue(cursor::MoveTo(
                self.highlightinfo.highlight_x,
                self.highlightinfo.highlight_y + i,
            ))?;
            self.stdout.queue(PrintStyledContent("█".cyan()))?;
        }

        Ok(())
    }

    fn render_task_scroll_bar(&mut self) -> Result<()> {
        // The worst attempt at a scrollbar you've ever laid eyes on
        // Genuinely no idea what I'm doing

        // Goal is to have a scroll bar to the right of the tasks so you know how many of them
        // you are seeing relative to all total tasks
        if self.taskinfo.display_tasklist.len() < self.taskwindow.tasks_that_can_fit as usize {
            return Ok(());
        }
        //let hidden_tasks_len =
        //    self.taskinfo.total_tasklist.len() - self.taskinfo.display_tasklist.len();
        //let hidden_tasks_size = hidden_tasks_len * self.task_height as usize;
        //let total_tasks_size = self.taskinfo.total_tasklist.len() * self.task_height as usize;
        //let total_over_hidden = total_tasks_size as f64 / hidden_tasks_size as f64;

        // Total height that the scrollbar can take up
        let scrollable_height = self.height - (self.box_padding * 2) - 2;

        //let scrollbar_height_to_tasks =
        //scrollable_height as f64 / self.taskinfo.display_tasklist.len() as f64;
        //let display_ratio = (self.taskinfo.display_tasklist.len() as f64)
        //    / self.taskinfo.total_tasklist.len() as f64;
        //let scrollbar_ratio = scrollable_height as f64 / amount_of_tasks as f64;
        //let reverse_ratio = amount_of_tasks as f64 / scrollable_height as f64;
        //let fix = scrollbar_ratio * self.taskwindow.tasks_that_can_fit as f64;
        //let extra_space = scrollable_height as i64
        //    - (self.taskinfo.display_tasklist.len() as i64 * self.task_height as i64)
        //    + 2;

        //let scrollbar_ratiod = (scrollable_height as f64 - extra_space as f64) * display_ratio;
        //let movement_ratiod = scrollbar_height_to_tasks * self.task_height as f64;
        //
        //let extra_space = scrollable_height as i64
        //    - (self.taskinfo.display_tasklist.len() as i64 * self.task_height as i64)
        //    + 2;
        //
        //let scrollbar = scrollable_height - hidden_tasks_size as u16;

        let bar_start = self.detail_box_start.1 as i64 + self.taskwindow.window_start;
        let bar_end = self.detail_box_start.1 as i64 + self.taskwindow.window_end;

        // Move to one space over from detail_box_start
        // Now render our scroll bar
        for i in bar_start..bar_end {
            if i > self.detail_box_start.1 as i64 + scrollable_height as i64 {
                // minor stopgap until I figure this out...
                continue;
            }
            self.stdout
                .queue(cursor::MoveTo(self.detail_box_start.0 - 1, i as u16))?;
            self.stdout.queue(PrintStyledContent("█".magenta()))?;
        }
        //for i in 0..=scrollable_height {
        //    // let adjustment = i * fraction;
        //    self.stdout.queue(cursor::MoveTo(
        //        scroll_start.0 - 1,
        //        self.detail_box_start.1 + i as u16,
        //    ))?;
        //    self.stdout.queue(PrintStyledContent("█".green()))?;
        //}

        Ok(())
    }

    fn resize_tasks_window(&mut self) {
        // Recalculate how many tasks we can show
        self.taskwindow.tasks_that_can_fit =
            ((self.height - (self.box_padding * 2)) / self.task_height) - 1;

        // If our resize allows us to display all our tasks
        if self.taskwindow.tasks_that_can_fit as usize >= self.taskinfo.total_tasklist.len() {
            self.taskwindow.window_start = 0;
            self.taskwindow.window_end = self.taskinfo.total_tasklist.len() as i64;
            self.highlightinfo.highlight_place = self.taskinfo.current_task;
        }
        // Otherwise we need to handle if after the resize our current task would be outside
        // of the task window
        else {
            // Current task would be greater than a new window if we just added tasks that
            // can fit to old start
            if self.taskinfo.current_task
                > self.taskwindow.window_start as u64 + self.taskwindow.tasks_that_can_fit as u64
            {
                self.taskwindow.window_end = self.taskinfo.current_task as i64;
                self.taskwindow.window_start =
                    self.taskwindow.window_end - self.taskwindow.tasks_that_can_fit as i64;
            }
            // Current task would be less than a new window if we removed tasks
            // that could fit to old end
            else if (self.taskinfo.current_task as i64)
                < self.taskwindow.window_end - self.taskwindow.tasks_that_can_fit as i64
            {
                self.taskwindow.window_start = self.taskinfo.current_task as i64;
                self.taskwindow.window_end =
                    self.taskwindow.window_start + self.taskwindow.tasks_that_can_fit as i64;
            }
            // otherwise, just create a new window of old start plus
            else {
                let potential_end =
                    self.taskwindow.window_start + self.taskwindow.tasks_that_can_fit as i64;
                if potential_end >= self.taskinfo.total_tasklist.len() as i64 {
                    self.taskwindow.window_end = self.taskinfo.total_tasklist.len() as i64 - 1;
                    self.taskwindow.window_start =
                        self.taskwindow.window_end - self.taskwindow.tasks_that_can_fit as i64;
                } else {
                    self.taskwindow.window_end = potential_end;
                }
            }
            // We can maintain highlight place by just taking our current task minus the window
            // start
            self.highlightinfo.highlight_place =
                self.taskinfo.current_task - self.taskwindow.window_start as u64;
        }
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
                    // Fix width and height
                    renderer.width = nw;
                    renderer.height = nh;

                    renderer.resize_tasks_window();

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
            if renderer.taskinfo.current_task != 0 {
                renderer.taskinfo.current_task -= 1;
                if (renderer.taskinfo.current_task as i64) < renderer.taskwindow.window_start {
                    renderer.taskwindow.window_start -= 1;
                    renderer.taskwindow.window_end -= 1;
                } else {
                    renderer.highlightinfo.highlight_place -= 1;
                }
            }
        }
        KeyCode::Down => {
            if renderer.taskinfo.current_task as usize + 1 != renderer.taskinfo.total_tasklist.len()
                && renderer.taskinfo.total_tasklist.len() != 0
            {
                renderer.taskinfo.current_task += 1;
                if renderer.taskinfo.current_task as i64 > renderer.taskwindow.window_end {
                    renderer.taskwindow.window_start += 1;
                    renderer.taskwindow.window_end += 1;
                } else {
                    renderer.highlightinfo.highlight_place += 1;
                }
            }
        }
        _ => panic!("We shouldn't be handling any other KeyCode here"),
    }
    renderer.render()?;
    Ok(())
}
