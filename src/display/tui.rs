use std::io::stdout;

use anyhow::{Context, Result};
use ratatui::crossterm::terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{execute, ExecutableCommand};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{
        palette::tailwind::{BLUE, GREEN, SLATE},
        Color, Modifier, Style, Stylize,
    },
    symbols,
    text::Line,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph,
        StatefulWidget, Widget, Wrap,
    },
    Terminal,
};
use rusqlite::Connection;

use crate::backend::database::{get_all_db_contents, get_db};
use crate::backend::task::{self, Status, Task, TaskList};

const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

struct CleanUp;

impl Drop for CleanUp {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Could not disable raw mode");
        execute!(stdout(), terminal::Clear(ClearType::All)).expect("Could not clear the screen");
        execute!(stdout(), LeaveAlternateScreen).expect("Could not leave alternate screen");
    }
}

pub fn init_terminal() -> std::io::Result<Terminal<impl Backend>> {
    stdout().execute(EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

pub fn run_tui(memory: bool, testing: bool) -> Result<()> {
    let _clean_up = CleanUp;
    let conn = get_db(memory, testing).context("Errored out making a database connection")?;
    let terminal = init_terminal()?;

    let mut app = App::new(memory, testing)?;
    app.run(terminal)?;

    Ok(())
}

struct TaskInfo {
    display_filter: task::Display,
    urgency_sort_desc: bool,
    tags_filter: Option<Vec<String>>,
}

impl TaskInfo {
    fn new() -> Self {
        Self {
            display_filter: task::Display::All,
            urgency_sort_desc: true,
            tags_filter: None,
        }
    }
}

struct App {
    should_exit: bool,
    conn: Connection,
    tasklist: TaskList,
    taskinfo: TaskInfo,
}

impl App {
    fn new(memory: bool, testing: bool) -> Result<Self> {
        let conn = get_db(memory, testing)?;
        let tasklist = TaskList::new();
        let taskinfo = TaskInfo::new();

        Ok(Self {
            should_exit: false,
            conn,
            tasklist,
            taskinfo,
        })
    }

    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> std::io::Result<()> {
        self.update_tasklist().unwrap();
        while !self.should_exit {
            terminal.draw(|f| f.render_widget(&mut *self, f.area()))?;
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            };
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('x') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('h') | KeyCode::Left => self.select_none(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            //KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            //    self.toggle_status();
            //}
            _ => {}
        }
    }

    fn select_none(&mut self) {
        self.tasklist.state.select(None);
    }

    fn select_next(&mut self) {
        self.tasklist.state.select_next();
    }
    fn select_previous(&mut self) {
        self.tasklist.state.select_previous();
    }

    fn select_first(&mut self) {
        self.tasklist.state.select_first();
    }

    fn select_last(&mut self) {
        self.tasklist.state.select_last();
    }

    fn update_tasklist(&mut self) -> Result<()> {
        // Get data
        let task_list = get_all_db_contents(&self.conn).unwrap();
        self.tasklist = task_list;

        // Filter tasks
        self.tasklist.filter_tasks(
            Some(self.taskinfo.display_filter),
            self.taskinfo.tags_filter.clone(),
        );

        // Order tasks here
        self.tasklist
            .sort_by_urgency(self.taskinfo.urgency_sort_desc);

        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let [list_area, item_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(main_area);

        App::render_header(header_area, buf);
        App::render_footer(footer_area, buf);
        self.render_list(list_area, buf);
        self.render_selected_item(item_area, buf);
    }
}

/// Rendering logic for the app
impl App {
    fn render_header(area: Rect, buf: &mut Buffer) {
        Paragraph::new("Welcome to your Checklist!")
            .bold()
            .centered()
            .render(area, buf);
    }

    fn render_footer(area: Rect, buf: &mut Buffer) {
        Paragraph::new("Actions: (a)dd    (u)pdate    (d)elete    e(x)it")
            .centered()
            .render(area, buf);
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(Line::raw("Tasks").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .tasklist
            .tasks
            .iter()
            .enumerate()
            .map(|(i, task_item)| {
                let color = alternate_colors(i);
                ListItem::from(task_item).bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.tasklist.state);
    }

    fn render_selected_item(&self, area: Rect, buf: &mut Buffer) {
        // We get the info depending on the item's state.
        let info = if let Some(i) = self.tasklist.state.selected() {
            match self.tasklist.tasks[i].status {
                Status::Completed => format!(
                    "✓ DONE: {}\n\nLatest Update: {}",
                    self.tasklist.tasks[i]
                        .description
                        .clone()
                        .unwrap_or("".to_string())
                        .blue(),
                    self.tasklist.tasks[i]
                        .latest
                        .clone()
                        .unwrap_or("".to_string())
                        .magenta()
                ),
                _ => format!(
                    "☐ TODO: {}\n\nLatest Update: {}",
                    self.tasklist.tasks[i]
                        .description
                        .clone()
                        .unwrap_or("".to_string())
                        .blue(),
                    self.tasklist.tasks[i]
                        .latest
                        .clone()
                        .unwrap_or("".to_string())
                        .magenta()
                ),
            }
        } else {
            "Nothing selected...".to_string()
        };

        // We show the list item's info under the list in this paragraph
        let block = Block::new()
            .title(Line::raw("Task Info").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG)
            .padding(Padding::horizontal(1));

        // We can now render the item info
        Paragraph::new(info)
            .block(block)
            //.fg(TEXT_FG_COLOR)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

const fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

impl From<&Task> for ListItem<'_> {
    fn from(value: &Task) -> Self {
        let line = match value.status {
            Status::Completed => Line::styled(
                format!(" ✓ - {} - {}", value.status.to_colored_string(), value.name),
                COMPLETED_TEXT_FG_COLOR,
            ),
            _ => Line::styled(
                format!(" ☐ - {} - {}", value.status.to_colored_string(), value.name),
                TEXT_FG_COLOR,
            ),
        };
        ListItem::new(line)
    }
}
