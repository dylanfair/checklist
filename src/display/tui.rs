use anyhow::{Context, Result};
use ratatui::layout::Alignment;
use ratatui::symbols::scrollbar;
use ratatui::widgets::{ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use ratatui::{
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout},
    style::{
        palette::tailwind::{BLUE, GREEN, SLATE},
        Color, Modifier, Style, Stylize,
    },
    text::{Line, Span},
    widgets::{Block, Borders, HighlightSpacing, List, ListItem, Paragraph, Scrollbar, Wrap},
    Terminal,
};
use rusqlite::Connection;

use crate::backend::database::{get_all_db_contents, get_db};
use crate::backend::task::{self, Status, Task, TaskList, Urgency};

use self::common::{init_terminal, install_hooks, restore_terminal};

//const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
//const TEXT_FG_COLOR: Color = SLATE.c200;
//const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

impl Status {
    fn to_colored_span(&self) -> Span<'_> {
        match self {
            Status::Open => String::from("Open").cyan(),
            Status::Working => String::from("Working").blue(),
            Status::Paused => String::from("Paused").yellow(),
            Status::Completed => String::from("Completed").green(),
        }
    }
}

impl Urgency {
    fn to_colored_span(&self) -> Span<'_> {
        match self {
            Urgency::Low => String::from("Low").green(),
            Urgency::Medium => String::from("Medium").light_yellow(),
            Urgency::High => String::from("High").yellow(),
            Urgency::Critical => String::from("Critical").red(),
        }
    }
}

impl Task {
    fn to_listitem(&self) -> ListItem {
        let line = match self.status {
            Status::Completed => {
                let spans = vec![
                    "✓ - ".green(),
                    self.status.to_colored_span().clone(),
                    " - ".into(),
                    self.name.clone().into(),
                ];
                Line::from(spans)
            }
            _ => {
                let spans = vec![
                    "☐ - ".white(),
                    self.status.to_colored_span().clone(),
                    " - ".into(),
                    self.name.clone().into(),
                ];
                Line::from(spans)
            }
        };
        ListItem::new(line)
    }

    fn to_paragraph(&self) -> Paragraph {
        todo!()
    }
}

pub fn run_tui(memory: bool, testing: bool) -> color_eyre::Result<(), anyhow::Error> {
    install_hooks()?;
    //let _clean_up = CleanUp;
    let conn = get_db(memory, testing).context("Errored out making a database connection")?;
    let terminal = init_terminal()?;

    let mut app = App::new(memory, testing)?;
    app.run(terminal)?;

    restore_terminal()?;

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
    vertical_scroll_state: ScrollbarState,
    vertical_scroll: usize,
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
            vertical_scroll_state: ScrollbarState::default(),
            vertical_scroll: 0,
        })
    }

    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> std::io::Result<()> {
        self.update_tasklist().unwrap();
        while !self.should_exit {
            terminal.draw(|f| ui(f, &mut *self))?;
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
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                self.adjust_scrollbar_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_previous();
                self.adjust_scrollbar_up();
            }
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            //KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            //    self.toggle_status();
            //}
            _ => {}
        }
    }

    fn adjust_scrollbar_down(&mut self) {
        self.vertical_scroll = self.vertical_scroll.saturating_add(1);
        self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
    }

    fn adjust_scrollbar_up(&mut self) {
        self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
        self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
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

const fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Percentage(5),  // Header/title
        Constraint::Percentage(90), // Main
        Constraint::Percentage(5),  // Footer
    ])
    .split(area);

    let information = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    app.vertical_scroll_state = app.vertical_scroll_state.content_length(app.tasklist.len());

    let title = Block::new()
        .title_alignment(Alignment::Left)
        .title("Welcome to your Checklist!");
    f.render_widget(title, chunks[0]);

    let footer = Paragraph::new("Actions: (a)dd    (u)pdate    (d)elete    e(x)it").centered();
    f.render_widget(footer, chunks[2]);

    // Now render our tasks
    let list_block = Block::new()
        .title(Line::raw("Tasks").left_aligned())
        .borders(Borders::ALL)
        //.border_set(symbols::border::EMPTY)
        //.border_style(TODO_HEADER_STYLE)
        .bg(NORMAL_ROW_BG);

    // Iterate through all elements in the `items` and stylize them.
    let items: Vec<ListItem> = app
        .tasklist
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task_item)| {
            let color = alternate_colors(i);
            let list_item = task_item.to_listitem();
            list_item.bg(color)
        })
        .collect();

    // Create a List from all list items and highlight the currently selected one
    let list = List::new(items)
        .block(list_block)
        .highlight_style(SELECTED_STYLE)
        .highlight_symbol(">")
        .highlight_spacing(HighlightSpacing::Always);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .symbols(scrollbar::VERTICAL)
        .begin_symbol(Some("↑"))
        .track_symbol(None)
        .end_symbol(Some("↓"));

    f.render_stateful_widget(list, information[0], &mut app.tasklist.state);
    //Now the scrollbar
    f.render_stateful_widget(
        scrollbar,
        information[0].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 0,
        }),
        &mut app.vertical_scroll_state,
    );

    let info = if let Some(i) = app.tasklist.state.selected() {
        match app.tasklist.tasks[i].status {
            _ => "Some text".to_string(), //Status::Completed => format!(
                                          //    "✓ DONE: {}\n\nLatest Update: {}",
                                          //    app.tasklist.tasks[i]
                                          //        .description
                                          //        .clone()
                                          //        .unwrap_or("".to_string())
                                          //        .blue(),
                                          //    app.tasklist.tasks[i]
                                          //        .latest
                                          //        .clone()
                                          //        .unwrap_or("".to_string())
                                          //        .magenta()
                                          //),
                                          //_ => format!(
                                          //    "☐ TODO: {}\n\nLatest Update: {}",
                                          //    app.tasklist.tasks[i]
                                          //        .description
                                          //        .clone()
                                          //        .unwrap_or("".to_string())
                                          //        .blue(),
                                          //    app.tasklist.tasks[i]
                                          //        .latest
                                          //        .clone()
                                          //        .unwrap_or("".to_string())
                                          //        .magenta()
                                          //),
        }
    } else {
        "Nothing selected...".to_string()
    };

    // We show the list item's info under the list in this paragraph
    let task_block = Block::new()
        .title(Line::raw("Task Info").centered())
        .borders(Borders::ALL)
        //.border_set(symbols::border::EMPTY)
        //.border_style(TODO_HEADER_STYLE)
        .bg(NORMAL_ROW_BG);
    //.padding(Padding::horizontal(1));

    // We can now render the item info
    let task_details = Paragraph::new(info)
        .block(task_block)
        //.scroll((app.vertical_scroll as u16, 0))
        //.fg(TEXT_FG_COLOR)
        .wrap(Wrap { trim: false });
    f.render_widget(task_details, information[1]);

    //self.render_list(list_area, buf);
    //self.render_selected_item(item_area, buf);
}

mod common {
    use std::{
        io::{self, stdout},
        panic,
    };

    use color_eyre::{
        config::{EyreHook, HookBuilder, PanicHook},
        eyre,
    };
    use ratatui::{
        backend::{Backend, CrosstermBackend},
        crossterm::{
            terminal::{
                disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
            },
            ExecutableCommand,
        },
        Terminal,
    };

    pub fn init_terminal() -> std::io::Result<Terminal<impl Backend>> {
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        Terminal::new(CrosstermBackend::new(stdout()))
    }

    /// Restore the terminal to its original state.
    pub fn restore_terminal() -> io::Result<()> {
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        Ok(())
    }

    /// Installs hooks for panic and error handling.
    ///
    /// Makes the app resilient to panics and errors by restoring the terminal before printing the
    /// panic or error message. This prevents error messages from being messed up by the terminal
    /// state.
    pub fn install_hooks() -> color_eyre::Result<(), anyhow::Error> {
        let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();
        install_panic_hook(panic_hook);
        install_error_hook(eyre_hook)?;
        Ok(())
    }

    /// Install a panic hook that restores the terminal before printing the panic.
    fn install_panic_hook(panic_hook: PanicHook) {
        let panic_hook = panic_hook.into_panic_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let _ = restore_terminal();
            panic_hook(panic_info);
        }));
    }

    /// Install an error hook that restores the terminal before printing the error.
    fn install_error_hook(eyre_hook: EyreHook) -> color_eyre::Result<(), anyhow::Error> {
        let eyre_hook = eyre_hook.into_eyre_hook();
        eyre::set_hook(Box::new(move |error| {
            let _ = restore_terminal();
            eyre_hook(error)
        }))?;
        Ok(())
    }
}