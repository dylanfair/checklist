use anyhow::{Context, Result};
use chrono::Local;
use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::backend::database::{add_to_db, update_task_in_db};
use crate::backend::task::{Status, Task, Urgency};
use crate::display::tui::{centered_ratio_rect, App};

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

#[derive(PartialEq, Eq)]
pub enum EntryMode {
    Add,
    Update,
}

impl Stage {
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
    pub fn from_task(&mut self, task: &Task) {
        self.name = task.name.clone();
        self.urgency = task.urgency;
        self.status = task.status;
        self.description = task.description.clone().unwrap_or("".to_string());
        self.latest = task.latest.clone().unwrap_or("".to_string());
        self.tags = task.tags.clone().unwrap_or(HashSet::new());
    }
}

impl App {
    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        let stage = if self.entry_mode == EntryMode::Add {
            &self.add_stage
        } else {
            &self.update_stage
        };

        match stage {
            Stage::Name => new_cursor_pos.clamp(0, self.inputs.name.chars().count()),
            Stage::Description => new_cursor_pos.clamp(0, self.inputs.description.chars().count()),
            Stage::Latest => new_cursor_pos.clamp(0, self.inputs.latest.chars().count()),
            Stage::Tags => new_cursor_pos.clamp(0, self.inputs.tags_input.chars().count()),
            _ => 0,
        }
    }

    fn byte_index(&self) -> usize {
        let stage = if self.entry_mode == EntryMode::Add {
            &self.add_stage
        } else {
            &self.update_stage
        };

        match stage {
            Stage::Name => self
                .inputs
                .name
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.character_index)
                .unwrap_or(self.inputs.name.len()),
            Stage::Description => self
                .inputs
                .description
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.character_index)
                .unwrap_or(self.inputs.description.len()),
            Stage::Latest => self
                .inputs
                .latest
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.character_index)
                .unwrap_or(self.inputs.latest.len()),
            Stage::Tags => self
                .inputs
                .tags_input
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.character_index)
                .unwrap_or(self.inputs.tags_input.len()),
            _ => 0,
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left)
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right)
    }

    fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();

        let stage = if self.entry_mode == EntryMode::Add {
            &self.add_stage
        } else {
            &self.update_stage
        };

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
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            let stage = if self.entry_mode == EntryMode::Add {
                &self.add_stage
            } else {
                &self.update_stage
            };

            match stage {
                Stage::Name => {
                    let before_char_to_delete =
                        self.inputs.name.chars().take(from_left_to_current_index);
                    let after_char_to_delete = self.inputs.name.chars().skip(current_index);
                    self.inputs.name = before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Description => {
                    let before_char_to_delete = self
                        .inputs
                        .description
                        .chars()
                        .take(from_left_to_current_index);
                    let after_char_to_delete = self.inputs.description.chars().skip(current_index);
                    self.inputs.description =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Latest => {
                    let before_char_to_delete =
                        self.inputs.latest.chars().take(from_left_to_current_index);
                    let after_char_to_delete = self.inputs.latest.chars().skip(current_index);
                    self.inputs.latest =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Tags => {
                    let before_char_to_delete = self
                        .inputs
                        .tags_input
                        .chars()
                        .take(from_left_to_current_index);
                    let after_char_to_delete = self.inputs.tags_input.chars().skip(current_index);
                    self.inputs.tags_input =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                _ => {}
            }
            self.move_cursor_left();
        }
    }

    pub fn handle_update_staging(&mut self, key: KeyEvent) {
        let current_index = self.tasklist.state.selected().unwrap();
        match key.code {
            KeyCode::Esc => self.update_popup = !self.update_popup,
            KeyCode::Char(ch) => {
                if ch == '1' {
                    self.update_stage = Stage::Name;
                    self.character_index = self.tasklist.tasks[current_index].name.len();
                }
                if ch == '2' {
                    self.update_stage = Stage::Status;
                }
                if ch == '3' {
                    self.update_stage = Stage::Urgency;
                }
                if ch == '4' {
                    self.update_stage = Stage::Description;
                    self.character_index = self.tasklist.tasks[current_index]
                        .description
                        .clone()
                        .unwrap_or("".to_string())
                        .len();
                }
                if ch == '5' {
                    self.update_stage = Stage::Latest;
                    self.character_index = self.tasklist.tasks[current_index]
                        .latest
                        .clone()
                        .unwrap_or("".to_string())
                        .len();
                }
                if ch == '6' {
                    self.update_stage = Stage::Tags;
                }
            }
            _ => {}
        }
    }

    pub fn handle_keys_for_text_inputs(&mut self, key: KeyEvent) {
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
                if self.entry_mode == EntryMode::Add {
                    self.add_stage.next();
                }
                if self.entry_mode == EntryMode::Update {
                    self.update_stage = Stage::Finished;
                }
            }
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
            KeyCode::Backspace => self.delete_char(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Char(ch) => self.enter_char(ch),
            _ => {}
        }
    }

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
                if self.inputs.tags_input == "".to_string() {
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
                    if self.inputs.tags.len() > 0 {
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
        task_tags_vec.sort_by(|a, b| a.cmp(b));

        // Get the value that is highlighted
        let tags_value = &task_tags_vec[self.tags_highlight_value];
        // Remove said value from our hashset
        self.inputs.tags.remove(tags_value);
        self.move_tags_highlight_left();

        if self.inputs.tags.len() == 0 {
            self.highlight_tags = false
        }
    }

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

    pub fn add_new_task_in(&mut self) -> Result<()> {
        let description = if self.inputs.description == "" {
            None
        } else {
            Some(self.inputs.description.clone())
        };
        let latest = if self.inputs.latest == "" {
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

    pub fn update_selected_task(&mut self) -> Result<()> {
        let current_selection = self.tasklist.state.selected().unwrap();
        let current_uuid = self.tasklist.tasks[current_selection].get_id();

        let description = if self.inputs.description == "" {
            None
        } else {
            Some(self.inputs.description.clone())
        };
        let latest = if self.inputs.latest == "" {
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

pub fn get_stage(f: &mut Frame, area: Rect) {
    let block = Block::bordered().title("Updating task");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("What do you want to update?"),
        Line::from(""),
        Line::from("1. Name"),
        Line::from("2. Status"),
        Line::from("3. Urgency"),
        Line::from("4. Description"),
        Line::from("5. Latest"),
        Line::from("6. Tags"),
    ]));

    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);
    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);
}

pub fn get_name(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered().title("Name");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("What do you want to name your task?"),
        Line::from(""),
        Line::from(app.inputs.name.as_str()),
    ]));

    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);
    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);
}

pub fn get_description(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered().title("Description");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("Feel free to add a description of your task"),
        Line::from(""),
        Line::from(app.inputs.description.as_str()),
    ]));

    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);
    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);
}

pub fn get_latest(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered().title("Latest Updates");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("Feel free to add an update if there is one"),
        Line::from(""),
        Line::from(app.inputs.latest.as_str()),
    ]));

    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);
    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);
}

pub fn get_urgency(f: &mut Frame, area: Rect) {
    let block = Block::new()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::TOP)
        .title("Urgency");
    let blurb = Paragraph::new(Text::from(vec![Line::from("What's the urgency level?")]));
    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);

    let urgencies = vec![
        ListItem::from(Line::from(vec![
            "1. ".into(),
            Urgency::Low.to_colored_span(),
        ])),
        ListItem::from(Line::from(vec![
            "2. ".into(),
            Urgency::Medium.to_colored_span(),
        ])),
        ListItem::from(Line::from(vec![
            "3. ".into(),
            Urgency::High.to_colored_span(),
        ])),
        ListItem::from(Line::from(vec![
            "4. ".into(),
            Urgency::Critical.to_colored_span(),
        ])),
    ];
    let urgencies_list = List::new(urgencies)
        .block(Block::new().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM))
        .bg(Color::Black);

    let popup_area = centered_ratio_rect(2, 3, area);

    let chunks =
        Layout::vertical([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(popup_area);

    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, chunks[0]);
    f.render_widget(urgencies_list, chunks[1]);
}

pub fn get_status(f: &mut Frame, area: Rect) {
    let block = Block::new()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .title("Status");
    let blurb = Paragraph::new(Text::from(vec![Line::from("What's the current status?")]));
    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);

    let statuses = vec![
        ListItem::from(Line::from(vec![
            "1. ".into(),
            Status::Open.to_colored_span(),
        ])),
        ListItem::from(Line::from(vec![
            "2. ".into(),
            Status::Working.to_colored_span(),
        ])),
        ListItem::from(Line::from(vec![
            "3. ".into(),
            Status::Paused.to_colored_span(),
        ])),
        ListItem::from(Line::from(vec![
            "4. ".into(),
            Status::Completed.to_colored_span(),
        ])),
    ];
    let status_list = List::new(statuses)
        .block(Block::new().borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT))
        .bg(Color::Black);

    let popup_area = centered_ratio_rect(2, 3, area);
    let chunks =
        Layout::vertical([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(popup_area);

    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, chunks[0]);
    f.render_widget(status_list, chunks[1]);
}

pub fn get_tags(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::new()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .title("Tags");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("Feel free to add any tags here"),
        Line::from("If there is any text, pressing enter will turn it into a tag"),
        Line::from("Pressing Down (↓) will highlight a tag, which you can delete with 'd'"),
        Line::from("Pressing Up (↑) will return you to text editing"),
        Line::from(""),
        Line::from(app.inputs.tags_input.as_str()),
    ]));
    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);

    let mut tags_span_vec = vec![];
    let mut task_tags_vec = Vec::from_iter(app.inputs.tags.clone());
    task_tags_vec.sort_by(|a, b| a.cmp(b));

    for (i, tag) in task_tags_vec.iter().enumerate() {
        let mut span_object = Span::from(format!(" {} ", tag).blue());
        if i == app.tags_highlight_value && app.highlight_tags {
            span_object = span_object.underlined();
        }
        tags_span_vec.push(span_object);
        tags_span_vec.push(Span::from("|"));
    }
    tags_span_vec.pop(); // removing the extra | at the end

    let tags_line = Line::from(tags_span_vec);
    let tags_blurb = Paragraph::new(Text::from(vec![tags_line]))
        .block(Block::new().borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT))
        .bg(Color::Black);

    let popup_area = centered_ratio_rect(2, 3, area);
    let chunks =
        Layout::vertical([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)]).split(popup_area);

    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, chunks[0]);
    f.render_widget(tags_blurb, chunks[1]);
}
