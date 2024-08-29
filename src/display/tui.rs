use anyhow::{Context, Result};
use crossterm::event::KeyModifiers;
use ratatui::symbols::scrollbar;
use ratatui::Frame;
use ratatui::{
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Alignment, Constraint, Layout, Rect},
    style::{
        palette::tailwind::{BLUE, GREEN, SLATE},
        Color, Modifier, Style, Stylize,
    },
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, HighlightSpacing, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Terminal,
};
use rusqlite::Connection;

use crate::backend::database::{add_to_db, delete_task_in_db, get_all_db_contents, get_db};
use crate::backend::task::{self, Status, Task, TaskList, Urgency};
use crate::display::add::{get_name, AddInputs, Stage};

use self::common::{init_terminal, install_hooks, restore_terminal};

use super::add::{get_description, get_latest, get_status, get_tags, get_urgency};

//const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
//const TEXT_FG_COLOR: Color = SLATE.c200;
//const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

impl Status {
    pub fn to_colored_span(&self) -> Span<'_> {
        match self {
            Status::Open => String::from("Open").cyan(),
            Status::Working => String::from("Working").blue(),
            Status::Paused => String::from("Paused").yellow(),
            Status::Completed => String::from("Completed").green(),
        }
    }
}

impl Urgency {
    pub fn to_colored_span(&self) -> Span<'_> {
        match self {
            Urgency::Low => String::from("Low").green(),
            Urgency::Medium => String::from("Medium").light_yellow(),
            Urgency::High => String::from("High").yellow(),
            Urgency::Critical => String::from("Critical").red(),
        }
    }
}

impl Task {
    fn span_tags(&self) -> Span {
        match &self.tags {
            Some(tags) => {
                let mut task_tags_vec = Vec::from_iter(tags);
                task_tags_vec.sort_by(|a, b| a.cmp(b));

                let mut tags_string = String::new();
                for tag in task_tags_vec {
                    tags_string.push_str(&format!("{} ", tag));
                }
                Span::from(tags_string.blue())
            }
            None => Span::from("".to_string()),
        }
    }

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
        let completion_date = match self.completed_on {
            Some(date) => format!(" - {}", date.date_naive().to_string()),
            None => String::from(""),
        };
        let text = vec![
            Line::from(vec![
                Span::styled("Title: ", Style::default()),
                Span::styled(&self.name, Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled("Created: ", Style::default()),
                Span::styled(
                    self.date_added.date_naive().to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default()),
                self.status.to_colored_span(),
                Span::styled(completion_date, Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("Urgency: ", Style::default()),
                self.urgency.to_colored_span(),
            ]),
            Line::from(vec![
                Span::styled("Tags: ", Style::default()),
                self.span_tags(),
            ]),
            Line::from(vec![Span::styled("", Style::default())]),
            Line::from(vec![Span::styled(
                "Latest: ",
                Style::default().underlined(),
            )]),
            Line::from(vec![Span::styled(
                self.latest.clone().unwrap_or("".to_string()),
                Style::default().fg(Color::Blue),
            )]),
            Line::from(vec![Span::styled("", Style::default())]),
            Line::from(vec![Span::styled(
                "Description: ",
                Style::default().underlined(),
            )]),
            Line::from(vec![Span::styled(
                self.description.clone().unwrap_or("".to_string()),
                Style::default().fg(Color::Magenta),
            )]),
        ];

        // let text = Text::from(lines);
        Paragraph::new(text)
    }
}

pub fn run_tui(memory: bool, testing: bool) -> color_eyre::Result<(), anyhow::Error> {
    install_hooks()?;
    //let _clean_up = CleanUp;
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

pub struct App {
    // Exit condition
    should_exit: bool,
    // DB connection
    conn: Connection,
    // Task related
    tasklist: TaskList,
    taskinfo: TaskInfo,
    // Scrollbar related
    vertical_scroll_state: ScrollbarState,
    vertical_scroll: usize,
    // Sizing related
    list_box_sizing: u16,
    // Popup related
    delete_popup: bool,
    // Add related
    pub add_popup: bool,
    pub add_stage: Stage,
    pub add_inputs: AddInputs,
    pub character_index: usize,
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
            list_box_sizing: 30,
            delete_popup: false,
            add_popup: false,
            add_stage: Stage::default(),
            add_inputs: AddInputs::default(),
            character_index: 0,
        })
    }

    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> std::io::Result<()> {
        match self.update_tasklist() {
            Ok(()) => {}
            Err(e) => panic!("Got an error dealing with update_tasklist(): {e:?}"),
        }
        while !self.should_exit {
            terminal.draw(|f| ui(f, &mut *self))?;
            if let Event::Key(key) = event::read()? {
                match self.handle_key(key) {
                    Ok(()) => {}
                    Err(e) => panic!("Got an error handling key: {key:?} - {e:?}"),
                }
            };
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if self.delete_popup {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let current_selection = self.tasklist.state.selected().unwrap();
                    delete_task_in_db(&self.conn, &self.tasklist.tasks[current_selection])?;
                    self.update_tasklist()?;

                    // Sets selector to where it would have been
                    if current_selection == 0 {
                        self.tasklist.state.select(Some(current_selection));
                    } else {
                        self.tasklist.state.select(Some(current_selection - 1));
                    }
                    self.delete_popup = !self.delete_popup
                }
                KeyCode::Char('n')
                | KeyCode::Char('N')
                | KeyCode::Char('x')
                | KeyCode::Esc
                | KeyCode::Backspace => self.delete_popup = !self.delete_popup,
                _ => {}
            }
            return Ok(());
        }

        if self.add_popup {
            match self.add_stage {
                Stage::Name => self.handle_keys_for_text_inputs(key),
                Stage::Urgency => self.handle_keys_for_urgency(key),
                Stage::Status => self.handle_keys_for_status(key),
                Stage::Description => self.handle_keys_for_text_inputs(key),
                Stage::Latest => self.handle_keys_for_text_inputs(key),
                Stage::Tags => self.handle_keys_for_tags(key),
                Stage::Finished => {
                    let description = if self.add_inputs.description == "" {
                        None
                    } else {
                        Some(self.add_inputs.description.clone())
                    };
                    let latest = if self.add_inputs.latest == "" {
                        None
                    } else {
                        Some(self.add_inputs.latest.clone())
                    };
                    let tags = if self.add_inputs.tags.is_empty() {
                        None
                    } else {
                        Some(self.add_inputs.tags.clone())
                    };

                    let task = Task::new(
                        self.add_inputs.name.clone(),
                        description,
                        latest,
                        Some(self.add_inputs.urgency),
                        Some(self.add_inputs.status),
                        tags,
                    );
                    add_to_db(&self.conn, &task)?;
                    self.add_popup = !self.add_popup
                }
            }
            return Ok(());
        }

        match key.modifiers {
            KeyModifiers::CONTROL => match key.code {
                KeyCode::Right => self.adjust_listbox_sizing_right(),
                KeyCode::Left => self.adjust_listbox_sizing_left(),
                KeyCode::Up => self.select_first(),
                KeyCode::Down => self.select_last(),
                _ => {}
            },
            KeyModifiers::NONE => {
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
                    KeyCode::Char('d') => match self.tasklist.state.selected() {
                        Some(_) => self.delete_popup = !self.delete_popup,
                        None => {}
                    },
                    KeyCode::Char('a') => {
                        self.add_popup = !self.add_popup;
                        self.add_inputs = AddInputs::default();
                    }
                    //KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                    //    self.toggle_status();
                    //}
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
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

    fn adjust_listbox_sizing_left(&mut self) {
        let new_size = self.list_box_sizing as i16 - 5;
        if new_size <= 20 {
            self.list_box_sizing = 20
        } else {
            self.list_box_sizing = new_size as u16
        }
    }

    fn adjust_listbox_sizing_right(&mut self) {
        let new_size = self.list_box_sizing as i16 + 5;
        if new_size >= 80 {
            self.list_box_sizing = 80
        } else {
            self.list_box_sizing = new_size as u16
        }
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

    let information = Layout::horizontal([
        Constraint::Percentage(app.list_box_sizing),
        Constraint::Percentage(100 - app.list_box_sizing),
    ])
    .split(chunks[1]);

    app.vertical_scroll_state = app.vertical_scroll_state.content_length(app.tasklist.len());

    let title = Block::new()
        .title_alignment(Alignment::Left)
        .title("Welcome to your Checklist!");
    f.render_widget(title, chunks[0]);

    let footer_text = Text::from(vec![
        Line::from("Actions: (a)dd (u)pdate (d)elete e(x)it"),
        Line::from("Adjust screen: CTRL ← or CTRL →"),
    ]);
    let footer = Paragraph::new(footer_text).centered();
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
            _ => app.tasklist.tasks[i].to_paragraph(),
        }
    } else {
        Paragraph::new("Nothing selected...")
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
    let task_details = info
        .block(task_block)
        //.scroll((app.vertical_scroll as u16, 0))
        //.fg(TEXT_FG_COLOR)
        .wrap(Wrap { trim: false });
    f.render_widget(task_details, information[1]);

    //self.render_list(list_area, buf);
    //self.render_selected_item(item_area, buf);
    if app.delete_popup {
        let delete_block = Block::bordered().title("Delete current task?");
        let blurb = Paragraph::new(Text::from(vec![
            Line::from("Are you sure you want to delete this task? (y)es (n)o"),
            //Line::from("(y)es (n)o"),
        ]));

        let delete_popup_contents = blurb
            .block(delete_block)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Center)
            .bg(Color::Black);
        let delete_popup_area = centered_ratio_rect(2, 3, area);
        f.render_widget(Clear, delete_popup_area);
        f.render_widget(delete_popup_contents, delete_popup_area);
    }

    if app.add_popup {
        match app.add_stage {
            Stage::Name => get_name(f, app, area),
            Stage::Urgency => get_urgency(f, app, area),
            Stage::Status => get_status(f, app, area),
            Stage::Description => get_description(f, app, area),
            Stage::Latest => get_latest(f, app, area),
            Stage::Tags => get_tags(f, app, area),
            Stage::Finished => {}
        }
    }
}

/// function that relies more on ratios to keep a centered rectangle
/// consitently sized based on terminal size
pub fn centered_ratio_rect(x_ratio: u32, y_ratio: u32, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Ratio(1, y_ratio * 2),
        Constraint::Ratio(1, y_ratio),
        Constraint::Ratio(1, y_ratio * 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Ratio(1, x_ratio * 2),
        Constraint::Ratio(1, x_ratio),
        Constraint::Ratio(1, x_ratio * 2),
    ])
    .split(popup_layout[1])[1]
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
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
