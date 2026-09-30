use ratatui::{
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget, Wrap},
};

use crate::structs_and_enums::{
    CleanState, MENU_ITEMS, ResultStatus, Row, RunState, Screen, SortOptions, Status,
};

use crate::theme::{
    ACCENT, ERR, MUTED, OK, focused_panel, footer_panel, highlight, key_hints, panel, title_bar};

use ratatui_spinner::LinearSpinner;

/// rendering the main screen function
pub fn render_menu(buf: &mut Buffer, state: &mut ListState, title: Rect, body: Rect, footer: Rect) {
    let descriptions = ["Find duplicate files", "Show the largest files in a folder"];
    let items = MENU_ITEMS
        .into_iter()
        .zip(descriptions)
        .map(|(name, desc)| {
            ListItem::new(vec![
                Line::from(name).bold(),
                Line::from(format!("  {desc}")).fg(MUTED),
            ])
        })
        .collect::<Vec<ListItem>>();
    let list = List::new(items)
        .highlight_style(highlight())
        .highlight_symbol(" ▶ ");

    title_bar("FS Scanner").render(title, buf);
    let outer_block = panel("Menu");
    let inner_area = outer_block.inner(body);
    outer_block.render(body, buf);
    StatefulWidget::render(list, inner_area, buf, state);

    let footer_block = footer_panel();
    let inner_footer = footer_block.inner(footer);
    footer_block.render(footer, buf);
    key_hints(&[
        ("j/↓", "down"),
        ("k/↑", "up"),
        ("Enter", "select"),
        ("q", "quit"),
    ])
    .render(inner_footer, buf);
}

/// rendering the clean options screen
pub fn render_clean(
    buf: &mut Buffer,
    options: &mut CleanState,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    title_bar("Clean Options").render(title, buf);

    let vertical_body = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ]);

    let [path, run, _] = vertical_body.areas(body);

    let valid = options.is_valid_path();
    let focused = options.focus == Row::Path;
    let badge = if valid {
        " ✔ valid ".fg(OK)
    } else {
        " ✖ invalid ".fg(ERR)
    };
    let mut para_block = panel(Row::Path.name()).title(Line::from(badge).right_aligned());
    if focused {
        para_block = para_block.border_style(Style::new().fg(if valid { OK } else { ERR }));
    }

    let input = if options.path.is_empty() {
        Line::from("e.g. /Users/you/Downloads").fg(MUTED)
    } else if focused {
        Line::from(format!("{}▏", options.path))
    } else {
        Line::from(options.path.as_str())
    };

    let inner_para = para_block.inner(path);
    para_block.render(path, buf);
    Paragraph::new(input).render(inner_para, buf);

    let run_label = Span::from("  ▶  Start scan  ");
    let (run_block, run_label) = if options.focus == Row::Run {
        (
            panel(Row::Run.name()).border_style(Style::new().fg(OK)),
            run_label.fg(Color::Black).bg(OK).bold(),
        )
    } else {
        (panel(Row::Run.name()), run_label.bold())
    };
    let inner_run = run_block.inner(run);
    run_block.render(run, buf);
    Line::from(run_label).centered().render(inner_run, buf);

    let footer_block = footer_panel();
    let inner_footer = footer_block.inner(footer);
    footer_block.render(footer, buf);

    let mut text = Vec::new();
    let line = match options.focus {
        Row::Path => key_hints(&[("Esc", "back"), ("Tab/↓", "down"), ("⇧Tab/↑", "up")]),
        _ => key_hints(&[("Esc", "back"), ("j/Tab/↓", "down"), ("k/⇧Tab/↑", "up")]),
    };

    let second_line = Line::from(options.focus.hint()).centered().fg(MUTED);
    text.push(line);
    text.push(second_line);

    if let Some(e) = &options.error {
        text.push(Line::from(format!("✖ {e}")).fg(ERR).centered())
    };

    Paragraph::new(text).render(inner_footer, buf);
}

// Loading screen when waiting for clean to finish
pub fn render_running_clean(
    title_str: &str,
    buf: &mut Buffer,
    options: &RunState,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    title_bar(title_str).render(title, buf);

    let spinner = LinearSpinner::new(options.tick)
        .total_slots(10)
        .active_color(ACCENT);

    let body_split = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]);
    let [loading_area, log_area] = body_split.areas(body);
    let loading_split = Layout::horizontal([
        Constraint::Length(
            (options.path.to_str().unwrap().len() + "Scanning Path:".len() + 5) as u16,
        ),
        Constraint::Min(0),
    ]);

    let [text, spinner_area] = loading_split.areas(loading_area);
    Line::from(vec![
        "Scanning Path: ".fg(MUTED),
        Span::from(options.path.display().to_string())
            .fg(ACCENT)
            .bold(),
    ])
    .render(text, buf);
    spinner.render(spinner_area, buf);

    // To render the logs, we take the last n lines of the log. We use log_area.height to use the
    // current height as an indicator of how many lines we can render. we use -2 since the area is
    // also surrounded by a border
    let shown = log_area.height.saturating_sub(2) as usize;
    let total = options.log_lines.len();
    let items = options
        .log_lines
        .iter()
        .rev()
        .take(shown)
        .rev()
        .enumerate()
        .map(|(i, log)| {
            // the newest line stays bright, older ones are dimmed
            let item = ListItem::new(log.as_str());
            if i + 1 == shown.min(total) {
                item
            } else {
                item.fg(MUTED)
            }
        })
        .collect::<Vec<ListItem>>();

    let log_title = format!("Logs ({total})");
    let log_block = panel(&log_title);
    let inner_log = log_block.inner(log_area);
    log_block.render(log_area, buf);

    Widget::render(List::new(items), inner_log, buf);

    Paragraph::new(key_hints(&[("q", "quit")]))
        .block(footer_panel())
        .render(footer, buf);
}

/// Display-only: in multi-line results (duplicates) the quoted paths stay bright and the
/// connecting words are dimmed. Single-line results are shown as is.
fn pretty_item(s: &str) -> ListItem<'_> {
    if !s.contains('\n') {
        return ListItem::new(s);
    }
    let mut lines = s
        .lines()
        .map(|l| {
            if l.trim_start().starts_with('"') {
                Line::from(l)
            } else {
                Line::from(l).fg(MUTED)
            }
        })
        .collect::<Vec<Line>>();
    lines.push(Line::from(""));
    ListItem::new(lines)
}

pub fn render_results_screen(
    screen: Option<Screen>,
    buf: &mut Buffer,
    results: &mut ResultStatus,
    state: &Status,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    match screen {
        Some(valid) => match valid {
            Screen::RunningClean => title_bar("Clean Results").render(title, buf),
            Screen::RunnignSort => title_bar("Sort Results").render(title, buf),
            _ => (),
        },
        _ => (),
    }

    if let Status::Failed(s) = state {
        Paragraph::new(vec![
            Line::from(""),
            Line::from(format!("✖ Got an error in the run: {s}"))
                .bold()
                .centered()
                .fg(ERR),
        ])
        .block(focused_panel("Error", ERR))
        .render(body, buf);
        return;
    }
    if results.output.is_none() {
        return;
    }

    let report = results.output.as_ref().unwrap();

    let succ_title = format!("✔ Success ({})", report.success.len());
    let err_title = format!("✖ Errors ({})", report.errors.len());
    let (succ_block, err_block) = if results.fail_focus {
        (panel(&succ_title), focused_panel(&err_title, ACCENT))
    } else {
        (focused_panel(&succ_title, ACCENT), panel(&err_title))
    };

    let output_split = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
    let [succ_area, fail_area] = output_split.areas(body);

    if report.success.is_empty() {
        let empty_msg = if screen == Some(Screen::RunnignSort) {
            "No files found"
        } else {
            "No duplicates found"
        };
        Paragraph::new(vec![Line::from(""), Line::from(empty_msg).fg(MUTED)])
            .centered()
            .block(succ_block)
            .render(succ_area, buf);
    } else {
        let succ_items = report
            .success
            .iter()
            .map(|(item, _)| pretty_item(item))
            .collect::<List>()
            .block(succ_block)
            .highlight_style(highlight())
            .highlight_symbol(" ▶ ");
        StatefulWidget::render(succ_items, succ_area, buf, &mut results.pos_succ);
    }

    if report.errors.is_empty() {
        Paragraph::new(vec![Line::from(""), Line::from("No errors ✔").fg(OK)])
            .centered()
            .block(err_block)
            .render(fail_area, buf);
    } else {
        let err_items = report
            .errors
            .iter()
            .map(|item| ListItem::new(item.as_str()))
            .collect::<List>()
            .block(err_block)
            .highlight_style(highlight())
            .highlight_symbol(" ▶ ");
        StatefulWidget::render(err_items, fail_area, buf, &mut results.pos_fail);
    }

    if results.is_popup {
        let (popup_block, popup_lines) = match &results.delete_msg {
            Some((Ok(msg), _)) => (
                focused_panel("✔ Deleted", OK),
                vec![Line::from(""), Line::from(format!("✔ {msg}")).fg(OK).bold()],
            ),
            Some((Err(msg), _)) => (
                focused_panel("✖ Delete failed", ERR),
                vec![Line::from(""), Line::from(format!("✖ {msg}")).fg(ERR).bold()],
            ),
            None => {
                let path = results
                    .pos_succ
                    .selected()
                    .and_then(|i| report.success.get(i))
                    .map(|(_, p)| format!("{:?}", p))
                    .unwrap_or_default();
                (
                    focused_panel("⚠ Delete this file?", ERR),
                    vec![
                        Line::from(""),
                        Line::from(path).bold(),
                        Line::from(""),
                        key_hints(&[("y", "yes, delete"), ("n", "cancel")]),
                    ],
                )
            }
        };
        let centered_area = body.centered(Constraint::Percentage(60), Constraint::Length(7));
        Widget::render(Clear, centered_area, buf);
        Paragraph::new(popup_lines)
            .centered()
            .wrap(Wrap { trim: true })
            .block(popup_block)
            .render(centered_area, buf);
    }

    let mut footer_lines = vec![
        key_hints(&[
            ("q", "quit"),
            ("j/↓", "down"),
            ("k/↑", "up"),
            ("Tab", "switch success/error"),
        ]),
        key_hints(&[("l", "logs"), ("Esc", "main menu")]),
    ];

    if !results.fail_focus
        && screen == Some(Screen::RunningClean)
        && let Some((_, path)) = results
            .pos_succ
            .selected()
            .and_then(|i| report.success.get(i))
    {
        let mut delete_line = key_hints(&[("d", "delete")]);
        delete_line.push_span(Span::from(format!("  {}", path.display())).fg(ERR));
        footer_lines.push(delete_line);
    }

    Paragraph::new(footer_lines)
        .block(footer_panel())
        .render(footer, buf);
}

/// A text input box: colored border when focused (green = valid, red = invalid), a ▏ cursor
/// when focused, and a dim placeholder when empty
fn input_field<'a>(
    block: Block<'a>,
    value: &'a str,
    placeholder: &'a str,
    focused: bool,
    valid: bool,
) -> Paragraph<'a> {
    let block = if focused {
        block.border_style(Style::new().fg(if valid { OK } else { ERR }))
    } else {
        block
    };
    let line = if value.is_empty() {
        Line::from(placeholder).fg(MUTED)
    } else if focused {
        Line::from(format!("{value}▏")).bold()
    } else {
        Line::from(value).bold()
    };
    Paragraph::new(line).block(block)
}

pub fn render_sort_options(
    buf: &mut Buffer,
    options: &SortOptions,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    title_bar("Sort Options").render(title, buf);

    let body_split = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ]);
    let [path_area, num_area, _] = body_split.areas(body);

    let path_valid = options.is_valid_path();
    let badge = if path_valid {
        " ✔ valid ".fg(OK)
    } else {
        " ✖ invalid ".fg(ERR)
    };
    input_field(
        panel("Path").title(Line::from(badge).right_aligned()),
        &options.path,
        "e.g. /Users/you/Downloads",
        options.focus.selected() == Some(0),
        path_valid,
    )
    .render(path_area, buf);

    input_field(
        panel("Number of Files to show"),
        &options.num_show,
        "e.g. 20",
        options.focus.selected() == Some(1),
        options.num_show.as_str().parse::<usize>().is_ok(),
    )
    .render(num_area, buf);

    let mut footer_lines = vec![key_hints(&[
        ("Esc", "main menu"),
        ("Enter", "run"),
        ("Tab/↓", "down"),
        ("⇧Tab/↑", "up"),
    ])];

    match &options.path_error {
        None => (),
        Some(e) => footer_lines.push(Line::from(format!("✖ {e}")).centered().fg(ERR)),
    }

    match &options.num_error {
        None => (),
        Some(e) => footer_lines.push(Line::from(format!("✖ {e}")).centered().fg(ERR)),
    }

    Paragraph::new(footer_lines)
        .block(footer_panel())
        .render(footer, buf);
}

pub fn render_log(
    title_str: &str,
    buf: &mut Buffer,
    logs: &[String],
    state: &mut ListState,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    title_bar(title_str).render(title, buf);

    let list: List = logs.iter().map(|log| ListItem::new(log.as_str())).collect();
    let list = list
        .highlight_symbol(" ▶ ")
        .highlight_style(highlight());
    let log_title = format!("Logs ({})", logs.len());
    let outer_block = panel(&log_title);
    let inner = outer_block.inner(body);

    outer_block.render(body, buf);

    StatefulWidget::render(list, inner, buf, state);

    Paragraph::new(key_hints(&[
        ("q", "quit"),
        ("Esc", "results"),
        ("j/↓", "down"),
        ("k/↑", "up"),
        ("PgUp/PgDn", "top/bottom"),
    ]))
    .block(footer_panel())
    .render(footer, buf);
}
