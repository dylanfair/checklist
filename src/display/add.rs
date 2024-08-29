use std::collections::HashSet;

use chrono::{DateTime, Local};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::backend::task::{Status, Urgency};
use crate::display::tui::{centered_ratio_rect, App};

#[derive(PartialEq, PartialOrd, Eq, Ord, Default)]
pub enum Stage {
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
    pub fn next(&self) -> Self {
        if *self == Stage::Name {
            Stage::Urgency
        } else if *self == Stage::Urgency {
            Stage::Status
        } else if *self == Stage::Status {
            Stage::Description
        } else if *self == Stage::Description {
            Stage::Latest
        } else if *self == Stage::Latest {
            Stage::Tags
        } else {
            Stage::Finished
        }
    }

    pub fn back(&self) -> Self {
        if *self == Stage::Finished {
            Stage::Tags
        } else if *self == Stage::Tags {
            Stage::Latest
        } else if *self == Stage::Latest {
            Stage::Description
        } else if *self == Stage::Description {
            Stage::Status
        } else if *self == Stage::Status {
            Stage::Urgency
        } else {
            Stage::Name
        }
    }
}

#[derive(Default)]
pub struct AddInputs {
    pub name: String,
    pub urgency: Urgency,
    pub status: Status,
    pub description: String,
    pub latest: String,
    pub tags: HashSet<String>,
    pub tags_input: String,
    pub date_added: DateTime<Local>,
    pub completed_on: Option<DateTime<Local>>,
}

impl App {
    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        match self.add_stage {
            Stage::Name => new_cursor_pos.clamp(0, self.add_inputs.name.chars().count()),
            Stage::Description => {
                new_cursor_pos.clamp(0, self.add_inputs.description.chars().count())
            }
            Stage::Latest => new_cursor_pos.clamp(0, self.add_inputs.latest.chars().count()),
            _ => 0,
        }
    }

    fn byte_index(&self) -> usize {
        match self.add_stage {
            Stage::Name => self
                .add_inputs
                .name
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.character_index)
                .unwrap_or(self.add_inputs.name.len()),
            Stage::Description => self
                .add_inputs
                .description
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.character_index)
                .unwrap_or(self.add_inputs.description.len()),
            Stage::Latest => self
                .add_inputs
                .latest
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.character_index)
                .unwrap_or(self.add_inputs.latest.len()),
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
        match self.add_stage {
            Stage::Name => self.add_inputs.name.insert(index, new_char),
            Stage::Description => self.add_inputs.description.insert(index, new_char),
            Stage::Latest => self.add_inputs.latest.insert(index, new_char),
            _ => {}
        }
        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            match self.add_stage {
                Stage::Name => {
                    let before_char_to_delete = self
                        .add_inputs
                        .name
                        .chars()
                        .take(from_left_to_current_index);
                    let after_char_to_delete = self.add_inputs.name.chars().skip(current_index);
                    self.add_inputs.name =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Description => {
                    let before_char_to_delete = self
                        .add_inputs
                        .description
                        .chars()
                        .take(from_left_to_current_index);
                    let after_char_to_delete =
                        self.add_inputs.description.chars().skip(current_index);
                    self.add_inputs.description =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                Stage::Latest => {
                    let before_char_to_delete = self
                        .add_inputs
                        .latest
                        .chars()
                        .take(from_left_to_current_index);
                    let after_char_to_delete = self.add_inputs.latest.chars().skip(current_index);
                    self.add_inputs.latest =
                        before_char_to_delete.chain(after_char_to_delete).collect();
                }
                _ => {}
            }
            self.move_cursor_left();
        }
    }

    pub fn handle_keys_for_text_inputs(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.add_popup = !self.add_popup,
            KeyCode::Enter => self.add_stage = self.add_stage.next(),
            KeyCode::Backspace => self.delete_char(),
            KeyCode::Left => {
                if key.modifiers == KeyModifiers::CONTROL {
                    self.add_stage = self.add_stage.back();
                } else {
                    self.move_cursor_left()
                }
            }
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Char(ch) => self.enter_char(ch),
            _ => {}
        }
    }

    pub fn handle_keys_for_tags(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.add_popup = !self.add_popup,
            KeyCode::Enter => {
                if self.add_inputs.tags_input == "".to_string() {
                    self.add_stage = self.add_stage.next();
                } else {
                    self.add_inputs
                        .tags
                        .insert(self.add_inputs.tags_input.to_string());
                    self.add_inputs.tags_input = "".to_string();
                }
            }
            KeyCode::Backspace => self.delete_char(),
            KeyCode::Left => {
                if key.modifiers == KeyModifiers::CONTROL {
                    self.add_stage = self.add_stage.back();
                } else {
                    self.move_cursor_left()
                }
            }
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Char(ch) => self.enter_char(ch),
            _ => {}
        }
    }

    pub fn handle_keys_for_urgency(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left => {
                if key.modifiers == KeyModifiers::CONTROL {
                    self.add_stage = self.add_stage.back();
                }
            }
            KeyCode::Esc => self.add_popup = !self.add_popup,
            KeyCode::Char(ch) => {
                if ch == '1' {
                    self.add_inputs.urgency = Urgency::Low;
                    self.add_stage = self.add_stage.next();
                }
                if ch == '2' {
                    self.add_inputs.urgency = Urgency::Medium;
                    self.add_stage = self.add_stage.next();
                }
                if ch == '3' {
                    self.add_inputs.urgency = Urgency::High;
                    self.add_stage = self.add_stage.next();
                }
                if ch == '4' {
                    self.add_inputs.urgency = Urgency::Critical;
                    self.add_stage = self.add_stage.next();
                }
            }
            _ => {}
        }
    }

    pub fn handle_keys_for_status(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left => {
                if key.modifiers == KeyModifiers::CONTROL {
                    self.add_stage = self.add_stage.back();
                }
            }
            KeyCode::Esc => self.add_popup = !self.add_popup,
            KeyCode::Char(ch) => {
                if ch == '1' {
                    self.add_inputs.status = Status::Open;
                    self.add_stage = self.add_stage.next();
                }
                if ch == '2' {
                    self.add_inputs.status = Status::Working;
                    self.add_stage = self.add_stage.next();
                }
                if ch == '3' {
                    self.add_inputs.status = Status::Paused;
                    self.add_stage = self.add_stage.next();
                }
                if ch == '4' {
                    self.add_inputs.status = Status::Completed;
                    self.add_stage = self.add_stage.next();
                }
            }
            _ => {}
        }
    }
}

pub fn get_name(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered().title("New task - Name");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("What do you want to name your task?"),
        Line::from(""),
        Line::from(app.add_inputs.name.as_str()),
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
    let block = Block::bordered().title("New task - Description");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("Feel free to add a description of your task"),
        Line::from(""),
        Line::from(app.add_inputs.description.as_str()),
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
    let block = Block::bordered().title("New task - Latest Updates");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("Feel free to add an update if there is one"),
        Line::from(""),
        Line::from(app.add_inputs.latest.as_str()),
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

pub fn get_urgency(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::new()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::TOP)
        .title("New task - Urgency");
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

pub fn get_status(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::new()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .title("New task - Status");
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
        .title("New task - Tags");
    let blurb = Paragraph::new(Text::from(vec![
        Line::from("Feel free to add any tags here"),
        Line::from("If there is any text, pressing enter will turn it into a tag"),
        Line::from(
            "If there is no text, pressing enter will finish the process and create your task!",
        ),
        Line::from(""),
        Line::from(app.add_inputs.tags_input.as_str()),
    ]));
    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left)
        .bg(Color::Black);

    let mut task_tags_vec = Vec::from_iter(app.add_inputs.tags.clone());
    task_tags_vec.sort_by(|a, b| a.cmp(b));
    let mut tags = vec![];

    for tag in task_tags_vec {
        let list_item = ListItem::from(Line::from(tag.blue()));
        tags.push(list_item);
    }

    let tags_list = List::new(tags)
        .block(Block::new().borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT))
        .bg(Color::Black);

    let popup_area = centered_ratio_rect(2, 3, area);
    let chunks =
        Layout::vertical([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(popup_area);

    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, chunks[0]);
    f.render_widget(tags_list, chunks[1]);
}
