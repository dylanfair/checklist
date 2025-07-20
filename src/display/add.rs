use std::collections::HashSet;

use anyhow::{Context, Result};
use chrono::Local;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::backend::database::{add_to_db, update_task_in_db};
use crate::backend::task::{Status, Task, Urgency};
use crate::display::text::HighlightDirection;
use crate::display::tui::App;

/// Enum to flag if the input being provided by the user
/// is in the context of adding a task, updating one, or
/// doing a "quick add".
#[derive(PartialEq, Eq)]
pub enum EntryMode {
    Add,
    Update,
    QuickAdd,
}

/// Enum to flag the stage we are at during the Add process.
#[derive(PartialEq, PartialOrd, Eq, Ord, Default)]
pub enum Stage {
    Staging,
    #[default]
    Name,
    Urgency,
    Status,
    Description,
    Latest,
    Tags,
    Finished,
}

impl Stage {
    /// Rotates forward through the stages
    /// Begins at Name, ends at Finished
    pub fn next(&mut self) {
        match self {
            Stage::Name => *self = Stage::Urgency,
            Stage::Urgency => *self = Stage::Status,
            Stage::Status => *self = Stage::Description,
            Stage::Description => *self = Stage::Latest,
            Stage::Latest => *self = Stage::Tags,
            Stage::Tags => *self = Stage::Finished,
            _ => {}
        }
    }

    /// Rotates backward through the Stages
    /// Begins at Finished, ends at Name
    pub fn back(&mut self) {
        match self {
            Stage::Finished => *self = Stage::Tags,
            Stage::Tags => *self = Stage::Latest,
            Stage::Latest => *self = Stage::Description,
            Stage::Description => *self = Stage::Status,
            Stage::Status => *self = Stage::Urgency,
            Stage::Urgency => *self = Stage::Name,
            _ => {}
        }
    }
}

/// Struct to capture the inputs provided by a user
#[derive(Default)]
pub struct Inputs {
    pub name: String,
    pub urgency: Urgency,
    pub status: Status,
    pub description: String,
    pub latest: String,
    pub tags: HashSet<String>,
    pub tags_input: String,
}

impl Inputs {
    /// Creates an `Inputs` struct based on a `Task` provided
    pub fn from_task(task: &Task) -> Self {
        Inputs {
            name: task.name.clone(),
            urgency: task.urgency,
            status: task.status,
            description: task.description.clone().unwrap_or("".to_string()),
            latest: task.latest.clone().unwrap_or("".to_string()),
            tags: task.tags.clone().unwrap_or_default(),
            tags_input: "".to_string(),
        }
    }
}

impl App {
    pub fn get_stage_off_entry_mode(&self) -> &Stage {
        match self.entry_mode {
            EntryMode::Add => &self.add_stage,
            EntryMode::QuickAdd => &self.add_stage,
            EntryMode::Update => &self.update_stage,
        }
    }

    pub fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        let stage = self.get_stage_off_entry_mode();

        match stage {
            Stage::Name => new_cursor_pos.clamp(0, self.inputs.name.chars().count()),
            Stage::Description => new_cursor_pos.clamp(0, self.inputs.description.chars().count()),
            Stage::Latest => new_cursor_pos.clamp(0, self.inputs.latest.chars().count()),
            Stage::Tags => new_cursor_pos.clamp(0, self.inputs.tags_input.chars().count()),
            _ => 0,
        }
    }

    fn byte_index(&self) -> usize {
        let stage = self.get_stage_off_entry_mode();

        match stage {
            Stage::Name => self
                .inputs
                .name
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.text_info.character_index)
                .unwrap_or(self.inputs.name.len()),
            Stage::Description => self
                .inputs
                .description
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.text_info.character_index)
                .unwrap_or(self.inputs.description.len()),
            Stage::Latest => self
                .inputs
                .latest
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.text_info.character_index)
                .unwrap_or(self.inputs.latest.len()),
            Stage::Tags => self
                .inputs
                .tags_input
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.text_info.character_index)
                .unwrap_or(self.inputs.tags_input.len()),
            _ => 0,
        }
    }

    fn move_cursor_left(&mut self) {
        if self.text_info.is_text_highlighted {
            let (left, _) = self.get_highlight_start_and_end();
            self.text_info.character_index = left;
        } else {
            let cursor_moved_left = self.text_info.character_index.saturating_sub(1);
            self.text_info.character_index = self.clamp_cursor(cursor_moved_left);
        }
    }

    fn move_cursor_right(&mut self) {
        if self.text_info.is_text_highlighted {
            let (_, right) = self.get_highlight_start_and_end();
            self.text_info.character_index = right;
        } else {
            let cursor_moved_right = self.text_info.character_index.saturating_add(1);
            self.text_info.character_index = self.clamp_cursor(cursor_moved_right);
        }
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();

        if self.text_info.is_text_highlighted {
            self.delete_char();
        }

        let stage = self.get_stage_off_entry_mode();

        match stage {
            Stage::Name => self.inputs.name.insert(index, new_char),
            Stage::Description => self.inputs.description.insert(index, new_char),
            Stage::Latest => self.inputs.latest.insert(index, new_char),
            Stage::Tags => self.inputs.tags_input.insert(index, new_char),
            _ => {}
        }
        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.text_info.character_index != 0;
        if is_not_cursor_leftmost {
            let right;
            let left;

            if self.text_info.is_text_highlighted {
                (left, right) = self.get_highlight_start_and_end();
            } else {
                right = self.text_info.character_index;
                left = right - 1;
            }

            let stage = self.get_stage_off_entry_mode();

            match stage {
                Stage::Name => {
                    let before_char_to_delete = self.inputs.name.chars().take(left);
                    let after_char_to_delete = self.inputs.name.chars().skip(right);
                    self.inputs.name = before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Description => {
                    let before_char_to_delete = self.inputs.description.chars().take(left);
                    let after_char_to_delete = self.inputs.description.chars().skip(right);
                    self.inputs.description =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Latest => {
                    let before_char_to_delete = self.inputs.latest.chars().take(left);
                    let after_char_to_delete = self.inputs.latest.chars().skip(right);
                    self.inputs.latest =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Tags => {
                    let before_char_to_delete = self.inputs.tags_input.chars().take(left);
                    let after_char_to_delete = self.inputs.tags_input.chars().skip(right);
                    self.inputs.tags_input =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                _ => {}
            }

            if !self.text_info.is_text_highlighted {
                self.move_cursor_left();
            } else {
                self.text_info.character_index = left;
            }
        }
    }

    /// Handles the `KeyEvent` when user is choosing what to update
    pub fn handle_update_staging(&mut self, key: KeyEvent) {
        let current_index = self.tasklist.state.selected().unwrap();
        match key.code {
            KeyCode::Esc => self.update_popup = !self.update_popup,
            KeyCode::Char(ch) => {
                if ch == '1' {
                    self.update_stage = Stage::Name;
                    self.text_info.character_index = self.tasklist.tasks[current_index].name.len();
                }
                if ch == '2' {
                    self.update_stage = Stage::Status;
                }
                if ch == '3' {
                    self.update_stage = Stage::Urgency;
                }
                if ch == '4' {
                    self.update_stage = Stage::Description;
                    self.text_info.character_index = self.tasklist.tasks[current_index]
                        .description
                        .clone()
                        .unwrap_or("".to_string())
                        .len();
                }
                if ch == '5' {
                    self.update_stage = Stage::Latest;
                    self.text_info.character_index = self.tasklist.tasks[current_index]
                        .latest
                        .clone()
                        .unwrap_or("".to_string())
                        .len();
                }
                if ch == '6' {
                    self.text_info.character_index = 0;
                    self.update_stage = Stage::Tags;
                }
            }
            _ => {}
        }
    }

    /// Handles the `KeyEvent` when user is providing text input
    pub fn handle_keys_for_text_inputs(&mut self, key: KeyEvent) {
        match key.modifiers {
            KeyModifiers::CONTROL => match key.code {
                KeyCode::Left => {
                    if self.entry_mode == EntryMode::Add {
                        self.add_stage.back();
                    }
                    if self.entry_mode == EntryMode::Update {
                        self.update_stage = Stage::Staging;
                    }
                }
                KeyCode::Char('a') => {
                    self.highlight_all();
                    return;
                }
                _ => {}
            },
            KeyModifiers::SHIFT => match key.code {
                KeyCode::Right => {
                    self.highlight_single_char(HighlightDirection::Right);
                    return;
                }
                KeyCode::Left => {
                    self.highlight_single_char(HighlightDirection::Left);
                    return;
                }
                KeyCode::Char(ch) => self.enter_char(ch),
                _ => {}
            },
            _ => match key.code {
                KeyCode::Esc => {
                    if self.entry_mode == EntryMode::Add || self.entry_mode == EntryMode::QuickAdd {
                        self.add_popup = !self.add_popup;
                    }
                    if self.entry_mode == EntryMode::Update {
                        self.update_popup = !self.update_popup;
                    }
                }
                KeyCode::Enter => {
                    if self.entry_mode == EntryMode::Add {
                        self.add_stage.next();
                    }
                    if self.entry_mode == EntryMode::Update {
                        self.update_stage = Stage::Finished;
                    }
                    if self.entry_mode == EntryMode::QuickAdd {
                        self.add_stage = Stage::Finished;
                    }
                    self.text_info.character_index = 0;
                }
                KeyCode::Right => self.move_cursor_right(),
                KeyCode::Left => self.move_cursor_left(),
                KeyCode::Backspace => self.delete_char(),
                KeyCode::Char(ch) => self.enter_char(ch),
                _ => {}
            },
        }

        if self.text_info.is_text_highlighted {
            self.text_info.is_text_highlighted = false;
        }
    }

    /// Handles the `KeyEvent` when the user is at the Tags `Stage`
    pub fn handle_keys_for_tags(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.entry_mode == EntryMode::Add {
                    self.add_popup = !self.add_popup;
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_popup = !self.update_popup;
                }
            }
            KeyCode::Enter => {
                if self.inputs.tags_input == *"".to_string() {
                    if self.entry_mode == EntryMode::Add {
                        self.add_stage.next();
                    }
                    if self.entry_mode == EntryMode::Update {
                        self.update_stage = Stage::Finished;
                    }
                } else {
                    self.inputs.tags.insert(self.inputs.tags_input.to_string());
                    self.inputs.tags_input = "".to_string();
                }
                self.text_info.character_index = 0;
            }
            _ => {}
        }
        if self.highlight_tags {
            match key.code {
                KeyCode::Left => {
                    if key.modifiers == KeyModifiers::CONTROL {
                        if self.entry_mode == EntryMode::Add {
                            self.add_stage.back();
                        }
                        if self.entry_mode == EntryMode::Update {
                            self.update_stage = Stage::Staging;
                        }
                    } else {
                        self.move_tags_highlight_left()
                    }
                }
                KeyCode::Right => {
                    // Move highlight to the right
                    self.move_tags_highlight_right()
                }
                KeyCode::Up => {
                    // Unhighlight tags and place cursor back to character_index
                    self.highlight_tags = !self.highlight_tags
                }
                KeyCode::Char('d') => {
                    // Remove the highlighted tag
                    self.remove_tag();
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Left => {
                    if key.modifiers == KeyModifiers::CONTROL {
                        if self.entry_mode == EntryMode::Add {
                            self.add_stage.back();
                        }
                        if self.entry_mode == EntryMode::Update {
                            self.update_stage = Stage::Staging;
                        }
                    } else {
                        self.move_cursor_left()
                    }
                }
                KeyCode::Right => {
                    self.move_cursor_right();
                }
                KeyCode::Down => {
                    if !self.inputs.tags.is_empty() {
                        self.highlight_tags = !self.highlight_tags;
                    }
                }
                KeyCode::Char(ch) => self.enter_char(ch),
                KeyCode::Backspace => self.delete_char(),
                _ => {}
            }
        }
    }

    fn move_tags_highlight_left(&mut self) {
        if self.tags_highlight_value > 0 {
            self.tags_highlight_value -= 1;
        }
    }

    fn move_tags_highlight_right(&mut self) {
        if self.tags_highlight_value < self.inputs.tags.len() - 1 {
            self.tags_highlight_value += 1;
        }
    }

    fn remove_tag(&mut self) {
        // Match what our displayed vectors are
        let mut task_tags_vec = Vec::from_iter(self.inputs.tags.clone());
        task_tags_vec.sort();

        // Get the value that is highlighted
        let tags_value = &task_tags_vec[self.tags_highlight_value];
        // Remove said value from our hashset
        self.inputs.tags.remove(tags_value);
        self.move_tags_highlight_left();

        if self.inputs.tags.is_empty() {
            self.highlight_tags = false
        }
    }

    /// Handles the `KeyEvent` when in the Urgency `Stage`
    pub fn handle_keys_for_urgency(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.entry_mode == EntryMode::Add {
                    self.add_popup = !self.add_popup;
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_popup = !self.update_popup;
                }
            }
            KeyCode::Left => {
                if self.entry_mode == EntryMode::Add {
                    self.add_stage.back();
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_stage = Stage::Staging;
                }
            }
            KeyCode::Char(ch) => {
                if ch == '1' {
                    self.inputs.urgency = Urgency::Low;
                } else if ch == '2' {
                    self.inputs.urgency = Urgency::Medium;
                } else if ch == '3' {
                    self.inputs.urgency = Urgency::High;
                } else if ch == '4' {
                    self.inputs.urgency = Urgency::Critical;
                } else {
                    return;
                }

                if self.entry_mode == EntryMode::Add {
                    self.add_stage.next();
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_stage = Stage::Finished;
                }
            }
            _ => {}
        }
    }

    /// Handles the `KeyEvent` when in the Status `Stage`
    pub fn handle_keys_for_status(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.entry_mode == EntryMode::Add {
                    self.add_popup = !self.add_popup;
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_popup = !self.update_popup;
                }
            }
            KeyCode::Left => {
                if self.entry_mode == EntryMode::Add {
                    self.add_stage.back();
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_stage = Stage::Staging;
                }
            }
            KeyCode::Char(ch) => {
                if ch == '1' {
                    self.inputs.status = Status::Open;
                } else if ch == '2' {
                    self.inputs.status = Status::Working;
                } else if ch == '3' {
                    self.inputs.status = Status::Paused;
                } else if ch == '4' {
                    self.inputs.status = Status::Completed;
                } else {
                    return;
                }

                if self.entry_mode == EntryMode::Add {
                    self.add_stage.next();
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_stage = Stage::Finished;
                }
            }
            _ => {}
        }
    }

    /// Adds a new `Task` into the SQLite database based on what is in
    /// the current `Inputs` struct in `App`.
    pub fn add_new_task_in(&mut self) -> Result<()> {
        let description = if self.inputs.description.is_empty() {
            None
        } else {
            Some(self.inputs.description.clone())
        };
        let latest = if self.inputs.latest.is_empty() {
            None
        } else {
            Some(self.inputs.latest.clone())
        };
        let tags = if self.inputs.tags.is_empty() {
            None
        } else {
            Some(self.inputs.tags.clone())
        };

        let new_task = Task::new(
            self.inputs.name.clone(),
            description,
            latest,
            Some(self.inputs.urgency),
            Some(self.inputs.status),
            tags,
        );

        add_to_db(&self.conn, &new_task).context("Failed to add the new task in")?;
        self.update_tasklist()
            .context("Failed to update the tasklist after adding the new task in")?;

        for (i, task) in self.tasklist.tasks.iter().enumerate() {
            if new_task.get_id() == task.get_id() {
                self.tasklist.state.select(Some(i))
            }
        }

        Ok(())
    }

    /// Updates a `Task` in the SQLite database that has been selected
    /// in the TUI.
    pub fn update_selected_task(&mut self) -> Result<()> {
        let current_selection = self.tasklist.state.selected().unwrap();
        let current_uuid = self.tasklist.tasks[current_selection].get_id();

        let description = if self.inputs.description.is_empty() {
            None
        } else {
            Some(self.inputs.description.clone())
        };
        let latest = if self.inputs.latest.is_empty() {
            None
        } else {
            Some(self.inputs.latest.clone())
        };
        let tags = if self.inputs.tags.is_empty() {
            None
        } else {
            Some(self.inputs.tags.clone())
        };

        self.tasklist.tasks[current_selection].name = self.inputs.name.clone();
        self.tasklist.tasks[current_selection].urgency = self.inputs.urgency;
        self.tasklist.tasks[current_selection].status = self.inputs.status;
        if self.tasklist.tasks[current_selection].status == Status::Completed {
            self.tasklist.tasks[current_selection].completed_on = Some(Local::now());
        } else {
            self.tasklist.tasks[current_selection].completed_on = None;
        }
        self.tasklist.tasks[current_selection].description = description;
        self.tasklist.tasks[current_selection].latest = latest;
        self.tasklist.tasks[current_selection].tags = tags;

        update_task_in_db(&self.conn, &self.tasklist.tasks[current_selection])
            .context("Failed to update task in the database")?;
        self.update_tasklist()
            .context("Failed to update the tasklist after adding the new task in")?;

        for (i, task) in self.tasklist.tasks.iter().enumerate() {
            if current_uuid == task.get_id() {
                self.tasklist.state.select(Some(i))
            }
        }

        Ok(())
    }
}
