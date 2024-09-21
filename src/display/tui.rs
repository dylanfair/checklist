use anyhow::Result;
use crossterm::event::KeyModifiers;
use ratatui::symbols::scrollbar;
use ratatui::Frame;
use ratatui::{
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Alignment, Constraint, Layout, Rect},
    style::{palette::tailwind::SLATE, Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, HighlightSpacing, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Terminal,
};
use rusqlite::Connection;

use crate::backend::config::Config;
use crate::backend::database::{delete_task_in_db, get_all_db_contents, get_db};
use crate::backend::task::{Status, Task, TaskList, Urgency};
use crate::display::add::{get_name, Inputs, Stage};

use self::common::{init_terminal, install_hooks, restore_terminal};

use super::add::{
    get_description, get_latest, get_stage, get_status, get_tags, get_urgency, EntryMode,
};

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
            Urgency::Medium => String::from("Medium").yellow(),
            Urgency::High => String::from("High").magenta(),
            Urgency::Critical => String::from("Critical").red(),
        }
    }

    pub fn to_colored_exclamation_marks(&self) -> Span<'_> {
        match self {
            Urgency::Low => String::from("   ").green(),
            Urgency::Medium => String::from("!  ").yellow(),
            Urgency::High => String::from("!! ").magenta(),
            Urgency::Critical => String::from("!!!").red(),
        }
    }
}

impl Task {
    fn span_tags(&self) -> Vec<Span> {
        let mut tags_span_vec = vec![Span::from("Tags:".to_string())];
        match &self.tags {
            Some(tags) => {
                let mut task_tags_vec = Vec::from_iter(tags);
                task_tags_vec.sort_by(|a, b| a.cmp(b));

                for tag in task_tags_vec {
                    tags_span_vec.push(Span::from(format!(" {} ", tag).blue()));
                    tags_span_vec.push(Span::from("|"));
                }
                tags_span_vec.pop(); // removing the extra | at the end
                tags_span_vec
            }
            None => tags_span_vec,
        }
    }

    fn to_listitem(&self) -> ListItem {
        let line = match self.status {
            Status::Completed => {
                let spans = vec![
                    "✓   | ".green(),
                    self.status.to_colored_span().clone(),
                    " - ".into(),
                    self.name.clone().into(),
                ];
                Line::from(spans)
            }
            _ => {
                let spans = vec![
                    //"☐ - ".white(),
                    self.urgency.to_colored_exclamation_marks(),
                    " | ".into(),
                    self.status.to_colored_span().clone(),
                    " - ".into(),
                    self.name.clone().into(),
                ];
                Line::from(spans)
            }
        };
        ListItem::new(line)
    }

    fn to_text_vec(&self) -> Vec<Line> {
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
            Line::from(self.span_tags()),
            Line::from(vec![Span::styled("", Style::default())]),
            Line::from(vec![Span::styled("Latest:", Style::default().underlined())]),
            Line::from(vec![Span::styled(
                self.latest.clone().unwrap_or("".to_string()),
                Style::default().fg(Color::Blue),
            )]),
            Line::from(vec![Span::styled("", Style::default())]),
            Line::from(vec![Span::styled(
                "Description:",
                Style::default().underlined(),
            )]),
            Line::from(vec![Span::styled(
                self.description.clone().unwrap_or("".to_string()),
                Style::default().fg(Color::Magenta),
            )]),
        ];
        text
    }

    fn to_paragraph(&self) -> Paragraph {
        let text = self.to_text_vec();

        Paragraph::new(text)
    }
}

pub fn run_tui(
    memory: bool,
    testing: bool,
    config: Config,
) -> color_eyre::Result<(), anyhow::Error> {
    install_hooks()?;
    //let _clean_up = CleanUp;
    let terminal = init_terminal()?;

    let mut app = App::new(memory, testing, config)?;
    app.run(terminal)?;

    restore_terminal()?;

    Ok(())
}

struct TaskInfo {
    tags_filter: Option<Vec<String>>,
}

impl TaskInfo {
    fn new() -> Self {
        Self { tags_filter: None }
    }
}

enum Runtime {
    Memory,
    Test,
    Real,
}

#[derive(Default)]
struct ScrollInfo {
    // list
    list_scroll_state: ScrollbarState,
    list_scroll: usize,
    // task info
    task_info_scroll_state: ScrollbarState,
    task_info_scroll: usize,
    // keys info
    keys_scroll_state: ScrollbarState,
    keys_scroll: usize,
}

pub struct App {
    // Exit condition
    should_exit: bool,
    // DB connection
    pub conn: Connection,
    // What type of database connection we have
    runtime: Runtime,
    // Config
    pub config: Config,
    // Task related
    pub tasklist: TaskList,
    taskinfo: TaskInfo,
    // Scrollbar related
    scroll_info: ScrollInfo,
    // Sizing related
    list_box_sizing: u16,
    // Popup related
    delete_popup: bool,
    // Entry related (add or update)
    pub entry_mode: EntryMode,
    // Add related
    pub add_popup: bool,
    pub add_stage: Stage,
    pub inputs: Inputs,
    pub character_index: usize,
    // Update related
    pub update_popup: bool,
    pub update_stage: Stage,
    // Tags related
    pub highlight_tags: bool,
    pub tags_highlight_value: usize,
    // Quick actions
    quick_action: bool,
}

impl App {
    fn new(memory: bool, testing: bool, config: Config) -> Result<Self> {
        let conn = get_db(memory, testing)?;
        let tasklist = TaskList::new();
        let taskinfo = TaskInfo::new();

        let runtime = if memory {
            Runtime::Memory
        } else if testing {
            Runtime::Test
        } else {
            Runtime::Real
        };

        Ok(Self {
            should_exit: false,
            conn,
            runtime,
            config,
            tasklist,
            taskinfo,
            scroll_info: ScrollInfo::default(),
            list_box_sizing: 30,
            delete_popup: false,
            entry_mode: EntryMode::Add,
            add_popup: false,
            add_stage: Stage::default(),
            inputs: Inputs::default(),
            character_index: 0,
            update_popup: false,
            update_stage: Stage::default(),
            highlight_tags: false,
            tags_highlight_value: 0,
            quick_action: false,
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
            match self.runtime {
                Runtime::Test => self.config.save(true).unwrap(),
                Runtime::Real => self.config.save(false).unwrap(),
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if self.quick_action {
            match key.code {
                KeyCode::Char('a') => {
                    // Let user choose a name, then make task
                    self.quick_add();
                    self.quick_action = !self.quick_action;
                    return Ok(());
                }
                KeyCode::Char('c') => {
                    self.quick_status()?;
                    self.quick_action = !self.quick_action;
                    return Ok(());
                }
                _ => {
                    self.quick_action = !self.quick_action;
                }
            }
        }

        if self.delete_popup {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('d') => {
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
                _ => {}
            }
            if self.add_stage == Stage::Finished {
                self.add_new_task_in()?;
                self.add_popup = !self.add_popup;
            }
            return Ok(());
        }

        if self.update_popup {
            match self.update_stage {
                Stage::Staging => self.handle_update_staging(key),
                Stage::Name => self.handle_keys_for_text_inputs(key),
                Stage::Urgency => self.handle_keys_for_urgency(key),
                Stage::Status => self.handle_keys_for_status(key),
                Stage::Description => self.handle_keys_for_text_inputs(key),
                Stage::Latest => self.handle_keys_for_text_inputs(key),
                Stage::Tags => self.handle_keys_for_tags(key),
                _ => {}
            }
            if self.update_stage == Stage::Finished {
                self.update_selected_task()?;
                self.update_popup = !self.update_popup;
            }
            return Ok(());
        }

        match key.modifiers {
            KeyModifiers::CONTROL => match key.code {
                KeyCode::Right => self.adjust_listbox_sizing_right(),
                KeyCode::Left => self.adjust_listbox_sizing_left(),
                KeyCode::Up => self.adjust_task_info_scrollbar_up(),
                KeyCode::Down => self.adjust_task_info_scrollbar_down(),
                _ => {}
            },
            KeyModifiers::ALT => match key.code {
                KeyCode::Up => self.adjust_keys_scrollbar_up(),
                KeyCode::Down => self.adjust_keys_scrollbar_down(),
                _ => {}
            },
            KeyModifiers::NONE => match key.code {
                KeyCode::Char('x') | KeyCode::Esc => self.should_exit = true,
                KeyCode::Char('s') => {
                    self.config.urgency_sort_desc = !self.config.urgency_sort_desc;
                    self.update_tasklist()?;
                }
                KeyCode::Char('f') => {
                    self.config.display_filter.next();
                    self.update_tasklist()?;
                }
                KeyCode::Char('h') | KeyCode::Left => self.select_none(),
                KeyCode::Char('j') | KeyCode::Down => {
                    self.select_next();
                    self.adjust_list_scrollbar_down();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.select_previous();
                    self.adjust_list_scrollbar_up();
                }
                KeyCode::Char('g') | KeyCode::Home => self.select_first(),
                KeyCode::Char('G') | KeyCode::End => self.select_last(),
                KeyCode::Char('d') => match self.tasklist.state.selected() {
                    Some(_) => self.delete_popup = !self.delete_popup,
                    None => {}
                },
                KeyCode::Char('a') => {
                    self.add_popup = !self.add_popup;
                    self.inputs = Inputs::default();
                    self.add_stage = Stage::Name;
                    self.entry_mode = EntryMode::Add;
                    self.highlight_tags = false;
                    self.tags_highlight_value = 0;
                }
                KeyCode::Char('u') => match self.tasklist.state.selected() {
                    Some(current_index) => {
                        self.update_popup = !self.update_popup;
                        self.entry_mode = EntryMode::Update;
                        self.update_stage = Stage::Staging;
                        self.highlight_tags = false;
                        self.tags_highlight_value = 0;
                        self.inputs.from_task(&self.tasklist.tasks[current_index])
                    }
                    None => {}
                },
                KeyCode::Char('q') => {
                    self.quick_action = !self.quick_action;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    fn adjust_list_scrollbar_down(&mut self) {
        self.scroll_info.list_scroll = self.scroll_info.list_scroll.saturating_add(1);
        self.scroll_info.list_scroll_state = self
            .scroll_info
            .list_scroll_state
            .position(self.scroll_info.list_scroll);
    }

    fn adjust_list_scrollbar_up(&mut self) {
        self.scroll_info.list_scroll = self.scroll_info.list_scroll.saturating_sub(1);
        self.scroll_info.list_scroll_state = self
            .scroll_info
            .list_scroll_state
            .position(self.scroll_info.list_scroll);
    }

    fn adjust_task_info_scrollbar_down(&mut self) {
        self.scroll_info.task_info_scroll = self.scroll_info.task_info_scroll.saturating_add(1);
        self.scroll_info.task_info_scroll_state = self
            .scroll_info
            .task_info_scroll_state
            .position(self.scroll_info.task_info_scroll);
    }

    fn adjust_task_info_scrollbar_up(&mut self) {
        self.scroll_info.task_info_scroll = self.scroll_info.task_info_scroll.saturating_sub(1);
        self.scroll_info.task_info_scroll_state = self
            .scroll_info
            .task_info_scroll_state
            .position(self.scroll_info.task_info_scroll);
    }

    fn adjust_keys_scrollbar_down(&mut self) {
        self.scroll_info.keys_scroll = self.scroll_info.keys_scroll.saturating_add(1);
        self.scroll_info.keys_scroll_state = self
            .scroll_info
            .keys_scroll_state
            .position(self.scroll_info.keys_scroll);
    }

    fn adjust_keys_scrollbar_up(&mut self) {
        self.scroll_info.keys_scroll = self.scroll_info.keys_scroll.saturating_sub(1);
        self.scroll_info.keys_scroll_state = self
            .scroll_info
            .keys_scroll_state
            .position(self.scroll_info.keys_scroll);
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

    pub fn update_tasklist(&mut self) -> Result<()> {
        // Get data
        let task_list = get_all_db_contents(&self.conn).unwrap();
        self.tasklist = task_list;

        // Filter tasks
        self.tasklist.filter_tasks(
            Some(self.config.display_filter),
            self.taskinfo.tags_filter.clone(),
        );

        // Order tasks here
        self.tasklist.sort_by_urgency(self.config.urgency_sort_desc);

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

fn render_keys(f: &mut Frame, app: &mut App, rectangle: &Rect) {
    // Render actions definitions
    let key_block = Block::new()
        .title(Line::raw("Keys").left_aligned())
        .borders(Borders::ALL)
        .bg(NORMAL_ROW_BG);

    let key_vec_lines = vec![
        Line::from("Actions:".underlined()),
        Line::from("a        - Add"),
        Line::from("u        - Update"),
        Line::from("d        - Delete"),
        Line::from("x or ESC - Exit"),
        Line::from("f        - Filter on Status"),
        Line::from("s        - Sort on Urgency"),
        Line::from(""),
        Line::from("Quick Actions:".underlined()),
        Line::from("qa       - Quick Add"),
        Line::from("qc       - Quick Complete"),
        Line::from(""),
        Line::from("Move/Adjustment:".underlined()),
        Line::from("↓ or j   - Move down task"),
        Line::from("↑ or k   - Move up task"),
        Line::from("CTRL ←   - Adjust screen left"),
        Line::from("CTRL →   - Adjust screen right"),
        Line::from("CTRL ↓   - Scroll Task Info down"),
        Line::from("CTRL ↑   - Scroll Task Info up"),
        Line::from("ALT ↓    - Scroll Keys down"),
        Line::from("ALT ↑    - Scroll Keys up"),
    ];
    let key_vec_lines_len = key_vec_lines.len();

    let key_text = Text::from(key_vec_lines);
    let key_paragraph = Paragraph::new(key_text)
        .block(key_block)
        .scroll((app.scroll_info.keys_scroll as u16, 0));

    f.render_widget(key_paragraph, *rectangle);

    // keys scrollbar
    app.scroll_info.keys_scroll_state = app
        .scroll_info
        .keys_scroll_state
        .content_length(key_vec_lines_len);

    let keys_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .symbols(scrollbar::VERTICAL)
        .begin_symbol(Some("↑"))
        .track_symbol(None)
        .end_symbol(Some("↓"));

    f.render_stateful_widget(
        keys_scrollbar,
        rectangle.inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 0,
        }),
        &mut app.scroll_info.keys_scroll_state,
    );
}

fn render_tasks(f: &mut Frame, app: &mut App, rectangle: &Rect) {
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

    let list_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .symbols(scrollbar::VERTICAL)
        .begin_symbol(Some("↑"))
        .track_symbol(None)
        .end_symbol(Some("↓"));

    f.render_stateful_widget(list, *rectangle, &mut app.tasklist.state);

    //Now the scrollbar
    app.scroll_info.list_scroll_state = app
        .scroll_info
        .list_scroll_state
        .content_length(app.tasklist.len());

    f.render_stateful_widget(
        list_scrollbar,
        rectangle.inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 0,
        }),
        &mut app.scroll_info.list_scroll_state,
    );
}

fn render_task_info(f: &mut Frame, app: &mut App, rectangle: &Rect) {
    let info = if let Some(i) = app.tasklist.state.selected() {
        match app.tasklist.tasks[i].status {
            _ => app.tasklist.tasks[i].to_paragraph(),
        }
    } else {
        Paragraph::new("Nothing selected...")
    };

    let text_len = app.tasklist.tasks[app.tasklist.state.selected().unwrap_or(0)]
        .to_text_vec()
        .len();

    // We show the list item's info under the list in this paragraph
    let task_block = Block::new()
        .title(Line::raw("Task Info"))
        .borders(Borders::ALL)
        //.border_set(symbols::border::EMPTY)
        //.border_style(TODO_HEADER_STYLE)
        .bg(NORMAL_ROW_BG);

    // We can now render the item info
    let task_details = info
        .block(task_block)
        .scroll((app.scroll_info.task_info_scroll as u16, 0))
        //.fg(TEXT_FG_COLOR)
        .wrap(Wrap { trim: false });
    f.render_widget(task_details, *rectangle);

    // Scrollbar
    app.scroll_info.task_info_scroll_state = app
        .scroll_info
        .task_info_scroll_state
        .content_length(text_len);

    let task_info_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .symbols(scrollbar::VERTICAL)
        .begin_symbol(Some("↑"))
        .track_symbol(None)
        .end_symbol(Some("↓"));

    f.render_stateful_widget(
        task_info_scrollbar,
        rectangle.inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 0,
        }),
        &mut app.scroll_info.task_info_scroll_state,
    );
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
        Constraint::Min(35),
    ])
    .split(chunks[1]);

    // Scroll states

    let title = Block::new()
        .title_alignment(Alignment::Left)
        .title("Welcome to your Checklist!");
    f.render_widget(title, chunks[0]);

    let urgency_sort_string = match app.config.urgency_sort_desc {
        true => "descending".to_string().blue(),
        false => "ascending".to_string().red(),
    };

    let footer_text = Text::from(vec![Line::from(format!(
        "Actions: (a)dd (u)pdate (d)elete e(x)it | current (f)ilter: {} | urgency (s)ort: {}",
        app.config.display_filter, urgency_sort_string
    ))]);
    let footer = Paragraph::new(footer_text).centered();
    f.render_widget(footer, chunks[2]);

    // Render tasks
    render_tasks(f, app, &information[0]);

    // Render task info
    render_task_info(f, app, &information[1]);

    // Render keys block
    render_keys(f, app, &information[2]);

    // popup renders
    // delete
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

    // add
    if app.add_popup {
        match app.add_stage {
            Stage::Name => get_name(f, app, area),
            Stage::Urgency => get_urgency(f, area),
            Stage::Status => get_status(f, area),
            Stage::Description => get_description(f, app, area),
            Stage::Latest => get_latest(f, app, area),
            Stage::Tags => get_tags(f, app, area),
            _ => {}
        }
    }

    if app.update_popup {
        match app.update_stage {
            Stage::Staging => get_stage(f, area),
            Stage::Name => get_name(f, app, area),
            Stage::Urgency => get_urgency(f, area),
            Stage::Status => get_status(f, area),
            Stage::Description => get_description(f, app, area),
            Stage::Latest => get_latest(f, app, area),
            Stage::Tags => get_tags(f, app, area),
            _ => {}
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
//fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
//    let popup_layout = Layout::vertical([
//        Constraint::Percentage((100 - percent_y) / 2),
//        Constraint::Percentage(percent_y),
//        Constraint::Percentage((100 - percent_y) / 2),
//    ])
//    .split(r);
//
//    Layout::horizontal([
//        Constraint::Percentage((100 - percent_x) / 2),
//        Constraint::Percentage(percent_x),
//        Constraint::Percentage((100 - percent_x) / 2),
//    ])
//    .split(popup_layout[1])[1]
//}

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
