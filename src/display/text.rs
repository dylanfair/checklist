use crate::display::tui::App;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

pub struct TextInfo {
    pub character_index: usize,
    pub is_text_highlighted: bool,
    pub highlight_info: HighLightInfo,
}

pub struct HighLightInfo {
    pub start: usize,
    pub distance: i32,
}

pub enum HighlightDirection {
    Left,
    Right,
}

impl TextInfo {
    pub fn new() -> Self {
        Self {
            character_index: 0,
            is_text_highlighted: false,
            highlight_info: HighLightInfo {
                start: 0,
                distance: 0,
            },
        }
    }
}

impl App {
    pub fn highlight_single_char(&mut self, direction: HighlightDirection) {
        if !self.text_info.is_text_highlighted {
            self.text_info.is_text_highlighted = true;
            self.text_info.highlight_info.start = self.text_info.character_index;
            self.text_info.highlight_info.distance = 0;
        }
        match direction {
            HighlightDirection::Left => {
                let cursor_moved_left = self.text_info.character_index.saturating_sub(1);
                self.text_info.character_index = self.clamp_cursor(cursor_moved_left);
                self.text_info.highlight_info.distance = self.text_info.character_index as i32
                    - self.text_info.highlight_info.start as i32;
            }
            HighlightDirection::Right => {
                let cursor_moved_right = self.text_info.character_index.saturating_add(1);
                self.text_info.character_index = self.clamp_cursor(cursor_moved_right);
                self.text_info.highlight_info.distance = self.text_info.character_index as i32
                    - self.text_info.highlight_info.start as i32;
            }
        }
    }
}

pub fn highlight_text(text: String, app: &App) -> Line {
    let start;
    let end;
    let tmp = app.text_info.highlight_info.start as i32 + app.text_info.highlight_info.distance;
    if tmp as usize > app.text_info.highlight_info.start {
        start = app.text_info.highlight_info.start;
        end = tmp as usize;
    } else {
        end = app.text_info.highlight_info.start;
        start = tmp as usize;
    }

    let pre_highlight = &text[0..start];
    let pre_highlight_span = Span::raw(pre_highlight.to_owned());
    let highlight = &text[start..end];
    let highlight_span = Span::styled(highlight.to_owned(), Style::default().bg(Color::Yellow));
    let post_highlight = &text[end..];
    let post_highlight_span = Span::raw(post_highlight.to_owned());
    Line::from(vec![
        pre_highlight_span,
        highlight_span,
        post_highlight_span,
    ])
}
