use ratatui::symbols::scrollbar;
use ratatui::widgets::BorderType;
use ratatui::Frame;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, HighlightSpacing, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, Wrap,
    },
};
use std::collections::BTreeMap;

use crate::backend::task::Display;
use crate::backend::task::{Status, Task, Urgency};
use crate::display::tui::{centered_ratio_rect, App, LayoutView};

impl Status {
    /// Based on the Enum value, will return a colored `Span`
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
    /// Based on the Enum value, will return a colored `Span`
    pub fn to_colored_span(&self) -> Span<'_> {
        match self {
            Urgency::Low => String::from("Low").green(),
            Urgency::Medium => String::from("Medium").yellow(),
            Urgency::High => String::from("High").magenta(),
            Urgency::Critical => String::from("Critical").red(),
        }
    }

    /// Based on the Enum value, will return a colored `Span` of exclamation marks
    pub fn to_colored_exclamation_marks(&self) -> Span<'_> {
        match self {
            Urgency::Low => String::from("   ").green(),
            Urgency::Medium => String::from("!  ").yellow(),
            Urgency::High => String::from("!! ").magenta(),
            Urgency::Critical => String::from("!!!").red(),
        }
    }
}

impl Display {
    /// Based on the Enum value, will return a colored `Span`
    pub fn to_colored_span(&self) -> Span<'_> {
        match self {
            Display::All => Span::styled(String::from("All"), Style::default().fg(Color::Cyan)),
            Display::Completed => {
                Span::styled(String::from("Completed"), Style::default().fg(Color::Green))
            }
            Display::NotCompleted => Span::styled(
                String::from("NotCompleted"),
                Style::default().fg(Color::Yellow),
            ),
        }
    }
}

impl LayoutView {
    /// Based on the Enum value, will return a colored `Span`
    pub fn to_colored_span(&self) -> Span<'_> {
        match self {
            LayoutView::Horizontal => {
                Span::styled(String::from("Horizontal"), Style::default().fg(Color::Cyan))
            }
            LayoutView::Vertical => {
                Span::styled(String::from("Vertical"), Style::default().fg(Color::Blue))
            }
            LayoutView::Smart => {
                Span::styled(String::from("Smart"), Style::default().fg(Color::Yellow))
            }
        }
    }
}

impl Task {
    /// Returns the `Task` tags as a vector of `Span`
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

    /// Returns a `ListItem` of the `Task`
    pub fn to_listitem(&self) -> ListItem {
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

    /// Returns a vector of `Line` containing several elements of the `Task`
    pub fn to_text_vec(&self) -> Vec<Line> {
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

    /// Returns a `Paragraph` of the `Task`. This is what is displayed
    /// in the `Task Info` block in the app
    pub fn to_paragraph(&self) -> Paragraph {
        let text = self.to_text_vec();

        Paragraph::new(text)
    }
}

const fn alternate_colors(i: usize, normal_color: Color, alternate_color: Color) -> Color {
    if i % 2 == 0 {
        normal_color
    } else {
        alternate_color
    }
}

fn map_string_to_lines(
    string: String,
    width_of_space: u16,
) -> (BTreeMap<usize, Vec<String>>, usize) {
    // Idea: create a BtreeMap where
    // keys - the line row
    // values - the line contents as a vector of strings (words)
    //
    // afterwards, we can use it to calculate where our cursor
    // needs to be based on app.character_index

    let mut quotients_seen = vec![0];
    let mut current_line_words = vec![];
    let mut word: String = String::new();

    let mut hash_lines: BTreeMap<usize, Vec<String>> = BTreeMap::from([(0, vec![])]);
    let mut latest_quotient = 0;

    for character in string.chars() {
        if character == ' ' {
            current_line_words.push(String::from(" "));
            word = String::new();
        } else {
            word.push(character.clone());
            if word.len() > 1 {
                current_line_words.pop(); // replace last word
            }
            current_line_words.push(word.clone());
        }
        hash_lines.insert(latest_quotient, current_line_words.clone());

        let total_chars: usize = hash_lines
            .iter()
            .map(|(_, v)| {
                v.iter()
                    .map(|x| {
                        if x == "OVER FLOW" {
                            return 1;
                        }
                        x.chars().count()
                    })
                    .sum::<usize>()
            })
            .sum();

        let new_character_quotient = total_chars / width_of_space as usize;

        if !quotients_seen.contains(&new_character_quotient) {
            if character == ' ' {
                // space gets "absorbed" in the box, so can use a blank vec
                current_line_words = vec![];
            } else {
                // correct prior line
                // pop off last line
                let latest_word = current_line_words.pop().unwrap();
                // add number of spaces based on length of word remaining
                let overflow_offset = latest_word.chars().count();
                for _ in 0..overflow_offset {
                    current_line_words.push(String::from("OVER FLOW"));
                }
                // insert it back in
                hash_lines.insert(latest_quotient, current_line_words.clone());

                // start a new curent_line with our word that overflowed
                current_line_words = vec![latest_word];
            }
            // insert newest into our hashmap
            hash_lines.insert(new_character_quotient, current_line_words.clone());
            quotients_seen.push(new_character_quotient);
            latest_quotient = new_character_quotient;
        }
    }

    (hash_lines, latest_quotient)
}

fn text_cursor_logic(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    current_string: String,
    x_offset: u16,
    y_offset: u16,
) {
    // Idea: create a BtreeMap where
    // keys - the line row
    // values - the line contents as a vector of strings (words)
    //
    // afterwards, we can use it to calculate where our cursor
    // needs to be based on app.character_index

    let text_start_x = area.left() + x_offset;
    let text_end_x = area.right();
    let text_start_y = area.top() + y_offset;

    let text_width = text_end_x - text_start_x;

    let (strings_on_lines, _) = map_string_to_lines(current_string, text_width);

    // Cursor logic - adjustment
    let mut x = app.character_index;
    let mut row = 0;

    if app.character_index > 0 {
        for (k, v) in strings_on_lines.iter() {
            let line_length: usize = v
                .iter()
                .map(|x| {
                    if x == "OVER FLOW" {
                        return 0;
                    }
                    x.chars().count()
                })
                .sum();
            row = *k;

            if x <= line_length {
                break;
            }
            x -= line_length;
        }
    }

    app.cursor_info.x = text_start_x + x as u16;
    app.cursor_info.y = text_start_y + row as u16;
    f.set_cursor_position(Position::new(app.cursor_info.x, app.cursor_info.y));
}

fn style_block(
    title: String,
    title_alignment: Alignment,
    bg_color: Color,
    outline_color: Color,
) -> Block<'static> {
    let block = Block::new()
        .title(Line::raw(title).alignment(title_alignment))
        .borders(Borders::ALL)
        .border_style(Style::new().fg(outline_color))
        .border_type(BorderType::Rounded)
        .bg(bg_color);

    block
}

fn style_two_halves_block(
    title: String,
    title_alignment: Alignment,
    bg_color: Color,
    outline_color: Color,
) -> (Block<'static>, Block<'static>) {
    let top_half = Block::new()
        .title(Line::raw(title.clone()).alignment(title_alignment))
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .border_style(Style::new().fg(outline_color))
        .border_type(BorderType::Rounded)
        .bg(bg_color);

    let bottom_half = Block::new()
        .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
        .border_style(Style::new().fg(outline_color))
        .border_type(BorderType::Rounded)
        .bg(bg_color);

    (top_half, bottom_half)
}

fn style_scrollbar<'a>(
    orientation: ScrollbarOrientation,
    color: Color,
    begin_symbol: Option<&'a str>,
    end_symbol: Option<&'a str>,
    thumb_symbol: Option<&'a str>,
    track_symbol: Option<&'a str>,
) -> Scrollbar<'a> {
    let styled_scrollbar = Scrollbar::new(orientation)
        .symbols(scrollbar::VERTICAL)
        .style(Style::new().fg(color))
        .begin_symbol(begin_symbol)
        .end_symbol(end_symbol)
        .thumb_symbol(thumb_symbol.unwrap())
        .track_symbol(track_symbol);

    styled_scrollbar
}

/// Renders the `State` block in the main TUI page
pub fn render_state(f: &mut Frame, app: &mut App, rectangle: Rect) {
    let urgency_sort_string = match app.config.urgency_sort_desc {
        true => Span::styled("descending".to_string(), Style::default().fg(Color::Blue)),
        false => Span::styled("ascending".to_string(), Style::default().fg(Color::Red)),
    };

    // Render actions definitions
    let mut state_block = style_block(
        "State".to_string(),
        Alignment::Left,
        app.theme.theme_colors.state_box_bg,
        app.theme.theme_colors.state_box_outline,
    );

    if app.enter_tags_filter {
        state_block = state_block
            .border_style(
                Style::new().fg(app.theme.theme_colors.state_box_outline_during_tags_edit),
            )
            .border_type(BorderType::Rounded);
    }

    let state_vec_lines = vec![
        Line::from("Filters:".underlined()),
        Line::from(vec![
            Span::styled("Status: ", Style::default()),
            app.config.display_filter.to_colored_span(),
        ]),
        Line::from(vec![
            Span::styled("Tag: ", Style::default()),
            app.tags_filter_value.clone().blue(),
        ]),
        Line::from(""),
        Line::from("Sorts:".underlined()),
        Line::from(vec![
            Span::styled("Urgency: ", Style::default()),
            urgency_sort_string,
        ]),
    ];

    let state_text = Text::from(state_vec_lines);
    let state_paragraph = Paragraph::new(state_text)
        .block(state_block)
        .wrap(Wrap { trim: false });

    f.render_widget(state_paragraph, rectangle);
}

/// Renders the `Help`
pub fn render_help(f: &mut Frame, app: &mut App, rectangle: Rect) {
    // Render actions definitions
    let help_block = style_block(
        "Help Menu".to_string(),
        Alignment::Center,
        app.theme.theme_colors.help_menu_bg,
        app.theme.theme_colors.help_menu_outline,
    );

    f.render_widget(Paragraph::new("").block(help_block), rectangle);

    let vertical_chunks = Layout::vertical([
        Constraint::Length(2), // Acts as a margin
        Constraint::Percentage(100),
        Constraint::Length(1), // Acts as a margin
    ])
    .split(rectangle);

    let horizontal_chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(vertical_chunks[1]);

    let mappings = vec![
        (
            vec!["Actions:".underlined().blue(), "         ".into()],
            "".into(),
        ),
        (vec!["a                ".into(), "".into()], "Add".blue()),
        (vec!["u                ".into(), "".into()], "Update".blue()),
        (vec!["d                ".into(), "".into()], "Delete".blue()),
        (
            vec!["x".into(), " or ".cyan(), "ESC         ".into(), "".into()],
            "Exit".blue(),
        ),
        (
            vec!["v                ".into(), "".into()],
            "Change layout view".blue(),
        ),
        (
            vec!["f                ".into(), "".into()],
            "Filter on Status".blue(),
        ),
        (
            vec!["/ <TEXT>         ".into(), "".into()],
            "Filter task on Tag".blue(),
        ),
        (
            vec!["/ ENTER          ".into(), "".into()],
            "Remove Tag filter".blue(),
        ),
        (
            vec!["s                ".into(), "".into()],
            "Sort on Urgency".blue(),
        ),
        (vec!["".into(), "".into()], "".into()),
        (
            vec!["Quick Actions:".underlined().magenta(), "   ".into()],
            "".into(),
        ),
        (
            vec!["qa               ".into(), "".into()],
            "Quick Add".magenta(),
        ),
        (
            vec!["qc               ".into(), "".into()],
            "Quick Complete".magenta(),
        ),
        (
            vec!["dd               ".into(), "".into()],
            "Quick Delete".magenta(),
        ),
        (vec!["".into(), "".into()], "".into()),
        (
            vec!["Move/Adjustment:".underlined().yellow(), " ".into()],
            "".into(),
        ),
        (
            vec!["↑".into(), " or ".cyan(), "k           ".into(), "".into()],
            "Move up task".yellow(),
        ),
        (
            vec!["↓".into(), " or ".cyan(), "j           ".into(), "".into()],
            "Move down task".yellow(),
        ),
        (
            vec!["HOME".into(), " or ".cyan(), "g        ".into(), "".into()],
            "Move to first task".yellow(),
        ),
        (
            vec!["END".into(), " or ".cyan(), "G         ".into(), "".into()],
            "Move to last task".yellow(),
        ),
        (
            vec!["CTRL ←           ".into(), "".into()],
            "Adjust Task Info pane (bigger)".yellow(),
        ),
        (
            vec!["CTRL →           ".into(), "".into()],
            "Adjust Task Info pane (smaller)".yellow(),
        ),
        (
            vec!["CTRL ↑".into(), " or ".cyan(), "k      ".into(), "".into()],
            "Scroll Task Info up".yellow(),
        ),
        (
            vec!["CTRL ↓".into(), " or ".cyan(), "j      ".into(), "".into()],
            "Scroll Task Info down".yellow(),
        ),
    ];
    let help_vec_lines_len = mappings.len();

    let mut titles = vec![];
    let mut values = vec![];
    for map in mappings {
        titles.push(Line::from(map.0));
        values.push(Line::from(map.1));
    }

    let titles_text = Text::from(titles);
    let titles_lines = Paragraph::new(titles_text)
        .block(Block::new())
        .alignment(Alignment::Right)
        .scroll((app.scroll_info.keys_scroll as u16, 0));

    let values_text = Text::from(values);
    let values_lines = Paragraph::new(values_text)
        .block(Block::new())
        .alignment(Alignment::Left)
        .scroll((app.scroll_info.keys_scroll as u16, 0));

    f.render_widget(titles_lines, horizontal_chunks[0]);
    f.render_widget(values_lines, horizontal_chunks[1]);

    // keys scrollbar
    app.scroll_info.keys_scroll_state = app
        .scroll_info
        .keys_scroll_state
        .content_length(help_vec_lines_len);

    let help_scrollbar = style_scrollbar(
        ScrollbarOrientation::VerticalRight,
        app.theme.theme_colors.help_menu_scrollbar,
        app.theme.theme_styles.scrollbar_begin.as_deref(),
        app.theme.theme_styles.scrollbar_end.as_deref(),
        app.theme.theme_styles.scrollbar_thumb.as_deref(),
        app.theme.theme_styles.scrollbar_track.as_deref(),
    );

    f.render_stateful_widget(
        help_scrollbar,
        horizontal_chunks[1].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 0,
        }),
        &mut app.scroll_info.keys_scroll_state,
    );
}

/// Renders the `Task` block in the TUI
pub fn render_tasks(f: &mut Frame, app: &mut App, rectangle: Rect) {
    // Now render our tasks
    let list_block = style_block(
        "Tasks".to_string(),
        Alignment::Left,
        app.theme.theme_colors.tasks_box_bg,
        app.theme.theme_colors.tasks_box_outline,
    );

    // Iterate through all elements in the `items` and stylize them.
    let items: Vec<ListItem> = app
        .tasklist
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task_item)| {
            let color = alternate_colors(
                i,
                app.theme.theme_colors.normal_row_bg,
                app.theme.theme_colors.alt_row_bg,
            );
            let list_item = task_item.to_listitem();
            list_item.bg(color)
        })
        .collect();

    // Create a List from all list items and highlight the currently selected one
    let list = List::new(items)
        .block(list_block)
        .highlight_style(
            Style::new()
                .bg(app.theme.theme_colors.selected_style)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&app.theme.theme_styles.highlight_symbol)
        .highlight_spacing(HighlightSpacing::Always);

    f.render_stateful_widget(list, rectangle, &mut app.tasklist.state);

    let list_scrollbar = style_scrollbar(
        ScrollbarOrientation::VerticalRight,
        app.theme.theme_colors.tasks_box_scrollbar,
        app.theme.theme_styles.scrollbar_begin.as_deref(),
        app.theme.theme_styles.scrollbar_end.as_deref(),
        app.theme.theme_styles.scrollbar_thumb.as_deref(),
        app.theme.theme_styles.scrollbar_track.as_deref(),
    );

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

/// Renders the `Task Info` block in the TUI
pub fn render_task_info(f: &mut Frame, app: &mut App, rectangle: Rect) {
    let info = if let Some(i) = app.tasklist.state.selected() {
        match app.tasklist.tasks[i].status {
            _ => app.tasklist.tasks[i].to_paragraph(),
        }
    } else {
        Paragraph::new("Nothing selected...")
    };

    let selected_task_len = match app.tasklist.state.selected() {
        Some(task) => app.tasklist.tasks[task].to_text_vec().len(),
        None => 0,
    };

    let task_block = style_block(
        "Task Info".to_string(),
        Alignment::Left,
        app.theme.theme_colors.tasks_info_box_bg,
        app.theme.theme_colors.tasks_info_box_outline,
    );

    // We can now render the item info
    let task_details = info
        .block(task_block)
        .scroll((app.scroll_info.task_info_scroll as u16, 0))
        //.fg(TEXT_FG_COLOR)
        .wrap(Wrap { trim: false });
    f.render_widget(task_details, rectangle);

    // Scrollbar
    app.scroll_info.task_info_scroll_state = app
        .scroll_info
        .task_info_scroll_state
        .content_length(selected_task_len);

    let task_info_scrollbar = style_scrollbar(
        ScrollbarOrientation::VerticalRight,
        app.theme.theme_colors.tasks_info_box_scrollbar,
        app.theme.theme_styles.scrollbar_begin.as_deref(),
        app.theme.theme_styles.scrollbar_end.as_deref(),
        app.theme.theme_styles.scrollbar_thumb.as_deref(),
        app.theme.theme_styles.scrollbar_track.as_deref(),
    );

    f.render_stateful_widget(
        task_info_scrollbar,
        rectangle.inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 0,
        }),
        &mut app.scroll_info.task_info_scroll_state,
    );
}

/// Renders the `Status Bar` in the TUI
pub fn render_status_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::horizontal([Constraint::Percentage(100), Constraint::Min(25)]).split(area);

    let help_blurb = if app.show_help {
        Paragraph::new(Text::from(vec![Line::from(vec![
            "Press (".into(),
            "ESC".cyan(),
            ") or (".into(),
            "h".cyan(),
            ") to return".into(),
        ])]))
    } else {
        Paragraph::new(Text::from(vec![Line::from(vec![
            "Press (".into(),
            "h".cyan(),
            ") for help".into(),
        ])]))
    };
    let help_contents = help_blurb
        .block(Block::new().bg(app.theme.theme_colors.status_bar))
        .alignment(Alignment::Left);

    let layout_blurb = Paragraph::new(Text::from(vec![Line::from(vec![
        "Layout View: ".into(),
        app.layout_view.to_colored_span(),
    ])]));
    let layout_contents = layout_blurb
        .block(Block::new().bg(app.theme.theme_colors.status_bar))
        .alignment(Alignment::Right);

    f.render_widget(help_contents, chunks[0]);
    f.render_widget(layout_contents, chunks[1]);
}

/// Renders the pop-up when deleting a `Task`
pub fn render_delete_popup(f: &mut Frame, app: &App, area: Rect) {
    let delete_block = style_block(
        "Delete current task?".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

    let blurb = Paragraph::new(Text::from(vec![
        Line::from("Are you sure you want to delete this task? (y)es (n)o"),
        //Line::from("(y)es (n)o"),
    ]));

    let delete_popup_contents = blurb
        .block(delete_block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Center)
        .bg(app.theme.theme_colors.pop_up_bg);

    let delete_popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, delete_popup_area);
    f.render_widget(delete_popup_contents, delete_popup_area);
}

/// Renders the pop-up when getting user input for what stage to update
pub fn render_stage_popup(f: &mut Frame, app: &App, area: Rect) {
    let block = style_block(
        "Updating task".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

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
        .alignment(Alignment::Left);

    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);
}

/// Renders the pop-up when getting user input for `Task` name
pub fn render_name_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let block = style_block(
        "Name".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

    let instructions = "What do you want to name your task?";

    let line_vec = vec![
        Line::from(instructions),
        Line::from(""),
        Line::from(app.inputs.name.as_str()),
    ];
    let line_vec_len = line_vec.len();
    let blurb = Paragraph::new(Text::from(line_vec));

    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);

    // If our text wraps, we want to start our cursor accordingly
    let text_width = popup_area.right() - popup_area.left() - 1;
    let (_, y_offset) = map_string_to_lines(instructions.to_string(), text_width);

    text_cursor_logic(
        f,
        app,
        popup_area,
        app.inputs.name.to_string(),
        1,
        line_vec_len as u16 + y_offset as u16,
    );
}

/// Renders the pop-up when getting user input for `Task` urgency
pub fn render_urgency_popup(f: &mut Frame, app: &App, area: Rect) {
    let (top_half, bottom_half) = style_two_halves_block(
        "Urgency".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

    let blurb = Paragraph::new(Text::from(vec![Line::from("What's the urgency level?")]));
    let popup_contents = blurb
        .block(top_half)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

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
    let urgencies_list = List::new(urgencies).block(bottom_half);

    let popup_area = centered_ratio_rect(2, 3, area);

    let chunks =
        Layout::vertical([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(popup_area);

    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, chunks[0]);
    f.render_widget(urgencies_list, chunks[1]);
}

/// Renders the pop-up when getting user input for `Task` status
pub fn render_status_popup(f: &mut Frame, app: &App, area: Rect) {
    let (top_half, bottom_half) = style_two_halves_block(
        "Status".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

    let blurb = Paragraph::new(Text::from(vec![Line::from("What's the current status?")]));
    let popup_contents = blurb
        .block(top_half)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

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
    let status_list = List::new(statuses).block(bottom_half);

    let popup_area = centered_ratio_rect(2, 3, area);
    let chunks =
        Layout::vertical([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(popup_area);

    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, chunks[0]);
    f.render_widget(status_list, chunks[1]);
}

/// Renders the pop-up when getting user input for `Task` description
pub fn render_description_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let block = style_block(
        "Description".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

    let instructions = "Feel free to add a description of your task";

    let line_vec = vec![
        Line::from(instructions),
        Line::from(""),
        Line::from(app.inputs.description.as_str()),
    ];
    let line_vec_len = line_vec.len();

    let blurb = Paragraph::new(Text::from(line_vec));

    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);

    // If our text wraps, we want to start our cursor accordingly
    let text_width = popup_area.right() - popup_area.left() - 1;
    let (_, y_offset) = map_string_to_lines(instructions.to_string(), text_width);

    text_cursor_logic(
        f,
        app,
        popup_area,
        app.inputs.description.to_string(),
        1,
        line_vec_len as u16 + y_offset as u16,
    );
}

/// Renders the pop-up when getting user input for `Task` latest
pub fn render_latest_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let block = style_block(
        "Latest Updates".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

    let instructions = "Feel free to add an update if there is one";
    let instructions_len = instructions.chars().count();
    let line_vec = vec![
        Line::from(instructions),
        Line::from(""),
        Line::from(app.inputs.latest.as_str()),
    ];
    let line_vec_len = line_vec.len();

    let blurb = Paragraph::new(Text::from(line_vec));

    let popup_contents = blurb
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    let popup_area = centered_ratio_rect(2, 3, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, popup_area);

    // If our text wraps, we want to start our cursor accordingly
    let text_width = popup_area.right() - popup_area.left() - 1;
    let y_offset = instructions_len as u16 / text_width;

    text_cursor_logic(
        f,
        app,
        popup_area,
        app.inputs.latest.to_string(),
        1,
        line_vec_len as u16 + y_offset,
    );
}

/// Renders the pop-up when getting user input for `Task` tags
pub fn render_tags_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let (top_half, bottom_half) = style_two_halves_block(
        "Tags".to_string(),
        Alignment::Center,
        app.theme.theme_colors.pop_up_bg,
        app.theme.theme_colors.pop_up_outline,
    );

    let popup_area = centered_ratio_rect(2, 2, area);
    let chunks =
        Layout::vertical([Constraint::Ratio(3, 4), Constraint::Ratio(1, 4)]).split(popup_area);

    let instructions = vec![
        "ENTER (with text) - create a tag",
        "ENTER (no text) - finish",
        "Pressing Down (↓) will highlight a tag, which you can delete with 'd'",
        "Pressing Up (↑) will return you to text editing",
    ];

    let text_width = popup_area.right() - popup_area.left() - 1;
    let mut line_vec = vec![];
    let mut final_y_offset = 0;
    for instruction in instructions {
        // If our text wraps, we want to start our cursor accordingly
        let (_, y_offset) = map_string_to_lines(instruction.to_string(), text_width);
        final_y_offset += y_offset;

        line_vec.push(Line::from(instruction))
    }

    line_vec.push(Line::from(""));
    line_vec.push(Line::from(app.inputs.tags_input.as_str()));
    let line_vec_len = line_vec.len();

    let blurb = Paragraph::new(Text::from(line_vec));
    let popup_contents = blurb
        .block(top_half)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

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
        .block(bottom_half)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    f.render_widget(Clear, popup_area);
    f.render_widget(popup_contents, chunks[0]);
    f.render_widget(tags_blurb, chunks[1]);

    text_cursor_logic(
        f,
        app,
        popup_area,
        app.inputs.tags_input.to_string(),
        1,
        line_vec_len as u16 + final_y_offset as u16,
    );
}
