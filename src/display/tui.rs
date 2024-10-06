use anyhow::Result;
use clap::ValueEnum;
use crossterm::event::KeyModifiers;
use ratatui::Frame;
use ratatui::{
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout},
    widgets::ScrollbarState,
    Terminal,
};
use rusqlite::Connection;

use crate::backend::config::Config;
use crate::backend::database::{delete_task_in_db, get_all_db_contents, get_db};
use crate::backend::task::TaskList;
use crate::display::add::{EntryMode, Inputs, Stage};
use crate::display::render::{
    render_delete_popup, render_description_popup, render_help, render_latest_popup,
    render_name_popup, render_stage_popup, render_state, render_status_bar, render_status_popup,
    render_tags_popup, render_task_info, render_tasks, render_urgency_popup,
};
use crate::display::theme::Theme;

use self::common::{init_terminal, install_hooks, restore_terminal};

pub fn run_tui(
    memory: bool,
    testing: bool,
    config: Config,
    theme: Theme,
    view: Option<LayoutView>,
) -> color_eyre::Result<(), anyhow::Error> {
    install_hooks()?;
    //let _clean_up = CleanUp;
    let terminal = init_terminal()?;

    let mut app = App::new(memory, testing, config, theme, view)?;
    app.run(terminal)?;

    restore_terminal()?;

    Ok(())
}

enum Runtime {
    Memory,
    Test,
    Real,
}

#[derive(Default, PartialEq, Eq, Debug, Clone, ValueEnum)]
pub enum LayoutView {
    Horizontal,
    Vertical,
    #[default]
    Smart,
}

impl LayoutView {
    fn next(&mut self) {
        match self {
            LayoutView::Smart => *self = LayoutView::Horizontal,
            LayoutView::Horizontal => *self = LayoutView::Vertical,
            LayoutView::Vertical => *self = LayoutView::Smart,
        }
    }
}

#[derive(Default)]
pub struct ScrollInfo {
    // list
    pub list_scroll_state: ScrollbarState,
    pub list_scroll: usize,
    // task info
    pub task_info_scroll_state: ScrollbarState,
    pub task_info_scroll: usize,
    // keys info
    pub keys_scroll_state: ScrollbarState,
    pub keys_scroll: usize,
}

#[derive(Default)]
pub struct CursorInfo {
    pub x: u16,
    pub y: u16,
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
    // Theme
    pub theme: Theme,
    // Layout View
    pub layout_view: LayoutView,
    // Cursor info
    pub cursor_info: CursorInfo,
    // Task related
    pub tasklist: TaskList,
    // Scrollbar related
    pub scroll_info: ScrollInfo,
    // Sizing related
    list_box_sizing: u16,
    // Popup related
    delete_popup: bool,
    // Entry related (add, quick_add, or update)
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
    // Tags filtering
    pub enter_tags_filter: bool,
    pub tags_filter_value: String,
    // Quick actions
    quick_action: bool,
    // Show help
    pub show_help: bool,
}

impl App {
    fn new(
        memory: bool,
        testing: bool,
        config: Config,
        theme: Theme,
        view: Option<LayoutView>,
    ) -> Result<Self> {
        let conn = get_db(memory, testing)?;
        let tasklist = TaskList::new();

        let runtime = if memory {
            Runtime::Memory
        } else if testing {
            Runtime::Test
        } else {
            Runtime::Real
        };

        let layout_view = view.unwrap_or_default();

        Ok(Self {
            should_exit: false,
            conn,
            runtime,
            config,
            theme,
            layout_view,
            cursor_info: CursorInfo::default(),
            tasklist,
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
            enter_tags_filter: false,
            tags_filter_value: String::new(),
            quick_action: false,
            show_help: false,
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

        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('h') => self.show_help = !self.show_help,
                KeyCode::Up | KeyCode::Char('k') => self.adjust_keys_scrollbar_up(),
                KeyCode::Down | KeyCode::Char('j') => self.adjust_keys_scrollbar_down(),
                _ => {}
            }
            return Ok(());
        }

        if self.enter_tags_filter {
            match key.code {
                KeyCode::Esc => {
                    self.enter_tags_filter = !self.enter_tags_filter;
                    self.tags_filter_value = String::new();
                }
                KeyCode::Enter => self.enter_tags_filter = !self.enter_tags_filter,
                KeyCode::Backspace => {
                    self.tags_filter_value.pop();
                }
                KeyCode::Char(ch) => {
                    self.tags_filter_value.push(ch);
                }
                KeyCode::Down => {
                    self.enter_tags_filter = !self.enter_tags_filter;
                    self.select_next();
                    self.adjust_list_scrollbar_down();
                }
                KeyCode::Up => {
                    self.enter_tags_filter = !self.enter_tags_filter;
                    self.select_previous();
                    self.adjust_list_scrollbar_up();
                }
                _ => {}
            }
            self.update_tasklist()?;
            return Ok(());
        }

        if self.quick_action {
            match key.code {
                KeyCode::Char('a') => {
                    // Let user choose a name, then make task
                    self.quick_add_setup();
                    self.quick_action = !self.quick_action;
                }
                KeyCode::Char('c') => {
                    self.quick_status()?;
                    self.quick_action = !self.quick_action;
                }
                _ => {
                    self.quick_action = !self.quick_action;
                }
            }
            return Ok(());
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
                KeyCode::Up | KeyCode::Char('k') => self.adjust_task_info_scrollbar_up(),
                KeyCode::Down | KeyCode::Char('j') => self.adjust_task_info_scrollbar_down(),
                _ => {}
            },
            KeyModifiers::SHIFT => match key.code {
                KeyCode::Char('G') => {
                    self.select_last();
                    self.adjust_list_scrollbar_last();
                }
                _ => {}
            },
            KeyModifiers::NONE => match key.code {
                KeyCode::Char('x') | KeyCode::Esc => self.should_exit = true,
                KeyCode::Char('v') => self.layout_view.next(),
                KeyCode::Char('s') => {
                    self.config.urgency_sort_desc = !self.config.urgency_sort_desc;
                    self.update_tasklist()?;
                }
                KeyCode::Char('f') => {
                    self.config.display_filter.next();
                    self.update_tasklist()?;
                }
                KeyCode::Left => self.select_none(),
                KeyCode::Char('h') => self.show_help = !self.show_help,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.select_next();
                    self.adjust_list_scrollbar_down();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.select_previous();
                    self.adjust_list_scrollbar_up();
                }
                KeyCode::Char('g') | KeyCode::Home => {
                    self.select_first();
                    self.adjust_list_scrollbar_first();
                }
                KeyCode::End => self.select_last(),
                KeyCode::Char('d') => {
                    if self.tasklist.state.selected().is_some() {
                        self.delete_popup = !self.delete_popup
                    }
                }
                KeyCode::Char('a') => {
                    self.add_popup = !self.add_popup;
                    self.inputs = Inputs::default();
                    self.character_index = 0;
                    self.add_stage = Stage::Name;
                    self.entry_mode = EntryMode::Add;
                    self.highlight_tags = false;
                    self.tags_highlight_value = 0;
                }
                KeyCode::Char('u') => {
                    if let Some(current_index) = self.tasklist.state.selected() {
                        self.update_popup = !self.update_popup;
                        self.entry_mode = EntryMode::Update;
                        self.update_stage = Stage::Staging;
                        self.highlight_tags = false;
                        self.tags_highlight_value = 0;
                        self.inputs = Inputs::from_task(&self.tasklist.tasks[current_index])
                    }
                }
                KeyCode::Char('q') => {
                    self.quick_action = !self.quick_action;
                }
                KeyCode::Char('/') => {
                    self.enter_tags_filter = !self.enter_tags_filter;
                    self.tags_filter_value = String::new();
                    self.update_tasklist()?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    fn adjust_list_scrollbar_up(&mut self) {
        self.scroll_info.list_scroll = self.scroll_info.list_scroll.saturating_sub(1);
        self.scroll_info.list_scroll_state = self
            .scroll_info
            .list_scroll_state
            .position(self.scroll_info.list_scroll);
    }

    fn adjust_list_scrollbar_down(&mut self) {
        self.scroll_info.list_scroll = self.scroll_info.list_scroll.saturating_add(1);
        self.scroll_info.list_scroll_state = self
            .scroll_info
            .list_scroll_state
            .position(self.scroll_info.list_scroll);
    }

    fn adjust_list_scrollbar_first(&mut self) {
        self.scroll_info.list_scroll = 0;
        self.scroll_info.list_scroll_state = self
            .scroll_info
            .list_scroll_state
            .position(self.scroll_info.list_scroll);
    }

    fn adjust_list_scrollbar_last(&mut self) {
        let task_len = self.tasklist.tasks.len();
        self.scroll_info.list_scroll = task_len;
        self.scroll_info.list_scroll_state = self.scroll_info.list_scroll_state.position(task_len);
    }

    fn adjust_task_info_scrollbar_up(&mut self) {
        self.scroll_info.task_info_scroll = self.scroll_info.task_info_scroll.saturating_sub(1);
        self.scroll_info.task_info_scroll_state = self
            .scroll_info
            .task_info_scroll_state
            .position(self.scroll_info.task_info_scroll);
    }

    fn adjust_task_info_scrollbar_down(&mut self) {
        self.scroll_info.task_info_scroll = self.scroll_info.task_info_scroll.saturating_add(1);
        self.scroll_info.task_info_scroll_state = self
            .scroll_info
            .task_info_scroll_state
            .position(self.scroll_info.task_info_scroll);
    }

    fn adjust_keys_scrollbar_up(&mut self) {
        self.scroll_info.keys_scroll = self.scroll_info.keys_scroll.saturating_sub(1);
        self.scroll_info.keys_scroll_state = self
            .scroll_info
            .keys_scroll_state
            .position(self.scroll_info.keys_scroll);
    }

    fn adjust_keys_scrollbar_down(&mut self) {
        self.scroll_info.keys_scroll = self.scroll_info.keys_scroll.saturating_add(1);
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
            self.tags_filter_value.clone(),
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
        if new_size >= 90 {
            self.list_box_sizing = 90
        } else {
            self.list_box_sizing = new_size as u16
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Percentage(100), // Main
        Constraint::Length(1),       // Status bar
    ])
    .split(area);

    if app.show_help {
        render_help(f, app, chunks[0]);
        render_status_bar(f, app, chunks[1])
    } else {
        let information = if app.layout_view == LayoutView::Vertical {
            Layout::vertical([
                Constraint::Percentage(app.list_box_sizing),
                Constraint::Percentage(100 - app.list_box_sizing),
                Constraint::Min(10),
            ])
            .split(chunks[0])
        } else if area.height < 32 || app.layout_view == LayoutView::Horizontal {
            Layout::horizontal([
                Constraint::Percentage(app.list_box_sizing),
                Constraint::Percentage(100 - app.list_box_sizing),
                Constraint::Min(25),
            ])
            .split(chunks[0])
        } else {
            // when LayoutView::Smart
            Layout::vertical([
                Constraint::Percentage(app.list_box_sizing),
                Constraint::Percentage(100 - app.list_box_sizing),
                Constraint::Min(10),
            ])
            .split(chunks[0])
        };

        // Render tasks
        render_tasks(f, app, information[0]);

        // Render task info
        render_task_info(f, app, information[1]);

        // Render task state
        render_state(f, app, information[2]);

        // Render status bar
        render_status_bar(f, app, chunks[1]);
    }

    // popup renders
    // delete
    if app.delete_popup {
        render_delete_popup(f, app, area);
    }

    // add
    if app.add_popup {
        match app.add_stage {
            Stage::Name => render_name_popup(f, app, area),
            Stage::Urgency => render_urgency_popup(f, app, area),
            Stage::Status => render_status_popup(f, app, area),
            Stage::Description => render_description_popup(f, app, area),
            Stage::Latest => render_latest_popup(f, app, area),
            Stage::Tags => render_tags_popup(f, app, area),
            _ => {}
        }
    }

    if app.update_popup {
        match app.update_stage {
            Stage::Staging => render_stage_popup(f, app, area),
            Stage::Name => render_name_popup(f, app, area),
            Stage::Urgency => render_urgency_popup(f, app, area),
            Stage::Status => render_status_popup(f, app, area),
            Stage::Description => render_description_popup(f, app, area),
            Stage::Latest => render_latest_popup(f, app, area),
            Stage::Tags => render_tags_popup(f, app, area),
            _ => {}
        }
    }
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
