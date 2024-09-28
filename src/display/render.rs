use ratatui::symbols::scrollbar;
use ratatui::Frame;
use ratatui::{
    layout::{Alignment, Rect},
    style::{palette::tailwind::SLATE, Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, HighlightSpacing, List, ListItem, Paragraph, Scrollbar,
        ScrollbarOrientation, Wrap,
    },
};

use crate::backend::task::Display;
use crate::display::tui::{centered_ratio_rect, App};

const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
//const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
//const TEXT_FG_COLOR: Color = SLATE.c200;
//const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

const fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

impl Display {
    pub fn to_colored_span(&self) -> Span<'_> {
        match self {
            Display::All => String::from("All").cyan(),
            Display::Completed => String::from("Completed").green(),
            Display::NotCompleted => String::from("NotCompleted").yellow(),
        }
    }
}

pub fn render_state(f: &mut Frame, app: &mut App, rectangle: &Rect) {
    let urgency_sort_string = match app.config.urgency_sort_desc {
        true => "descending".to_string().blue(),
        false => "ascending".to_string().red(),
    };

    // Render actions definitions
    let mut state_block = Block::new()
        .title(Line::raw("State").left_aligned())
        .borders(Borders::ALL)
        .bg(NORMAL_ROW_BG);

    if app.enter_tags_filter {
        state_block = state_block.border_style(Style::new().blue());
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

    f.render_widget(state_paragraph, *rectangle);
}

pub fn render_keys(f: &mut Frame, app: &mut App, rectangle: &Rect) {
    // Render actions definitions
    let key_block = Block::new()
        .title(Line::raw("Keys").left_aligned())
        .borders(Borders::ALL)
        .bg(NORMAL_ROW_BG);

    let key_vec_lines = vec![
        Line::from("Actions:".underlined()),
        Line::from("a        - Add".blue()),
        Line::from("u        - Update".blue()),
        Line::from("d        - Delete".blue()),
        Line::from("x or ESC - Exit".blue()),
        Line::from("f        - Filter on Status".blue()),
        Line::from("/ <TEXT> - Filter task on Tag".blue()),
        Line::from("/ ENTER  - Remove Tag filter".blue()),
        Line::from("s        - Sort on Urgency".blue()),
        Line::from(""),
        Line::from("Quick Actions:".underlined()),
        Line::from("qa       - Quick Add".magenta()),
        Line::from("qc       - Quick Complete".magenta()),
        Line::from("dd       - Quick Delete".magenta()),
        Line::from(""),
        Line::from("Move/Adjustment:".underlined()),
        Line::from("↓ or j   - Move down task".yellow()),
        Line::from("↑ or k   - Move up task".yellow()),
        Line::from("g or HOME- Move to first task".yellow()),
        Line::from("G or END - Move to last task".yellow()),
        Line::from("CTRL ←   - Adjust screen left".yellow()),
        Line::from("CTRL →   - Adjust screen right".yellow()),
        Line::from("CTRL ↑   - Scroll Task Info up".yellow()),
        Line::from("CTRL ↓   - Scroll Task Info down".yellow()),
        Line::from("SHIFT ↑  - Scroll Keys up".yellow()),
        Line::from("SHIFT ↓  - Scroll Keys down".yellow()),
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

pub fn render_tasks(f: &mut Frame, app: &mut App, rectangle: &Rect) {
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

pub fn render_task_info(f: &mut Frame, app: &mut App, rectangle: &Rect) {
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
        .content_length(selected_task_len);

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

pub fn render_delete_popup(f: &mut Frame, area: Rect) {
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
