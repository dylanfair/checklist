use std::io::{stdout, Stdout, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::Print;
use crossterm::terminal::{self, ClearType};
use crossterm::{cursor, execute, QueueableCommand};

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

    let mut renderer = Renderer::new(3);
    renderer.render()?;

    while run(memory, testing, &mut renderer)? {}

    Ok(())
}

struct Renderer {
    width: u16,
    height: u16,
    box_padding: u16,
    stdout: Stdout,
}

impl Renderer {
    fn new(box_padding: u16) -> Self {
        let (width, height) = terminal::size().unwrap();
        let stdout = stdout();
        Self {
            width,
            height,
            box_padding,
            stdout,
        }
    }

    fn render(&mut self) -> Result<()> {
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
}

fn run(memory: bool, testing: bool, renderer: &mut Renderer) -> Result<bool> {
    // Get data
    let conn = get_db(memory, testing).context("Errored out making a database connection")?;
    let mut task_list = get_all_db_contents(&conn).unwrap();

    // Filter tasks
    task_list.filter_tasks(Some(Display::NotCompleted), None);

    // Order tasks here
    task_list.sort_by_urgency(true);

    task_list.display_tasks();

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
