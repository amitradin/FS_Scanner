use ratatui::{
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget},
};

use crate::structs_and_enums::{
    CleanState, MENU_ITEMS, ResultStatus, Row, RunState, Screen, SortOptions, Status,
};

use ratatui_spinner::LinearSpinner;

/// rendering the main screen function
pub fn render_menu(buf: &mut Buffer, state: &mut ListState, title: Rect, body: Rect, footer: Rect) {
    let items = MENU_ITEMS
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<ListItem>>();
    let list = List::new(items)
        .highlight_style(Style::new().reversed())
        .highlight_symbol(">>");

    Line::from("FS Scanner")
        .bold()
        .centered()
        .render(title, buf);
    let outer_block = Block::bordered();
    let inner_area = outer_block.inner(body);
    outer_block.render(body, buf);
    StatefulWidget::render(list, inner_area, buf, state);

    let footer_block = Block::bordered();
    let inner_footer = footer_block.inner(footer);
    footer_block.render(footer, buf);
    Line::from("J - Down       K - Up       Enter - Select")
        .bold()
        .centered()
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
    Line::from("Clean Options")
        .bold()
        .centered()
        .render(title, buf);

    let vertical_body = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ]);

    let [path, run, _] = vertical_body.areas(body);

    let mut para_block = Block::bordered().title(Row::Path.name());
    if options.focus == Row::Path {
        if options.is_valid_path() {
            para_block = para_block.border_style(Style::new().green());
        } else {
            para_block = para_block.border_style(Style::new().red());
        }
    }

    let inner_para = para_block.inner(path);
    para_block.render(path, buf);
    Paragraph::new(options.path.as_str()).render(inner_para, buf);

    let mut run_block = Block::bordered();
    if options.focus == Row::Run {
        run_block = run_block.border_style(Style::new().green());
    }
    let inner_run = run_block.inner(run);
    run_block.render(run, buf);
    Line::from("Run").render(inner_run, buf);

    let footer_block = Block::bordered();
    let inner_footer = footer_block.inner(footer);
    footer_block.render(footer, buf);

    let mut text = Vec::new();
    let line = match options.focus {
        Row::Path => {
            Line::from("ESC- Go back       Tab/DownArrow - Down       BackTab/UpArrow - Up")
                .centered()
                .bold()
        }
        _ => Line::from("ESC- Go back       J/Tab/DownArrow - Down       K/BackTab/UpArrow - Up")
            .centered()
            .bold(),
    };

    let second_line = Line::from(options.focus.hint()).centered().bold();
    text.push(line);
    text.push(second_line);

    if let Some(e) = &options.error {
        text.push(Line::from(e.as_str().red()).centered())
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
    Line::from(title_str).bold().centered().render(title, buf);

    let spinner = LinearSpinner::new(options.tick).total_slots(10);

    let body_split = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]);
    let [loading_area, log_area] = body_split.areas(body);
    let loading_split = Layout::horizontal([
        Constraint::Length(
            (options.path.to_str().unwrap().len() + "Scanning Path:".len() + 5) as u16,
        ),
        Constraint::Min(0),
    ]);

    let [text, spinner_area] = loading_split.areas(loading_area);
    Line::from(format!("Scanning Path: {:?}", options.path))
        .bold()
        .render(text, buf);
    spinner.render(spinner_area, buf);

    // To render the logs, we take the last n lines of the log. We use log_area.height to use the
    // current height as an indicator of how many lines we can render. we use -2 since the area is
    // also surrounded by a border
    let items = options
        .log_lines
        .iter()
        .rev()
        .take((log_area.height.saturating_sub(2)) as usize)
        .rev()
        .map(|log| ListItem::new(log.as_str()))
        .collect::<Vec<ListItem>>();

    let log_block = Block::bordered().title("Logs");
    let inner_log = log_block.inner(log_area);
    log_block.render(log_area, buf);

    Widget::render(List::new(items), inner_log, buf);

    Paragraph::new(Line::from("q - quit").bold().centered())
        .block(Block::bordered())
        .render(footer, buf);
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
            Screen::RunningClean => Line::from("Clean Results")
                .bold()
                .centered()
                .render(title, buf),
            Screen::RunnignSort => Line::from("Sort Results")
                .bold()
                .centered()
                .render(title, buf),
            _ => (),
        },
        _ => (),
    }

    if let Status::Failed(s) = state {
        Paragraph::new(
            Line::from(format!("Got an error in the run: {s}"))
                .bold()
                .centered()
                .red(),
        )
        .block(Block::bordered())
        .render(body, buf);
        return;
    }
    if results.output.is_none() {
        return;
    }

    let report = results.output.as_ref().unwrap();

    let succ_items = report
        .success
        .iter()
        .map(|(item, _)| ListItem::new(item.as_str()))
        .collect::<List>();

    let mut succ_items = succ_items
        .block(Block::bordered().title("Success Output"))
        .highlight_style(Style::new().reversed())
        .highlight_symbol(">>");

    let err_items = report
        .errors
        .iter()
        .map(|item| ListItem::new(item.as_str()))
        .collect::<List>();

    let mut err_items = err_items
        .block(Block::bordered().title("Error Output"))
        .highlight_style(Style::new().reversed())
        .highlight_symbol(">>");

    if results.fail_focus {
        err_items = err_items.block(
            Block::bordered()
                .title("Error Output")
                .border_style(Style::new().green()),
        );
    } else {
        succ_items = succ_items.block(
            Block::bordered()
                .title("Success Output")
                .border_style(Style::new().green()),
        );
    }

    let output_split = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
    let [succ_area, fail_area] = output_split.areas(body);

    StatefulWidget::render(succ_items, succ_area, buf, &mut results.pos_succ);
    StatefulWidget::render(err_items, fail_area, buf, &mut results.pos_fail);

    if results.is_popup {
        let popup_block = Block::bordered()
            .title("Are you sure you want to delete?")
            .style(Style::new().red());
        let centered_area = body.centered(Constraint::Percentage(30), Constraint::Percentage(20));
        Widget::render(Clear, centered_area, buf);
        Paragraph::new("y - yes   n - no")
            .centered()
            .bold()
            .block(popup_block)
            .render(centered_area, buf);
    }

    let mut footer_lines = vec![Line::from(String::from(
        "q - quit       J/DownArrow - Move Down       K/UpArrow - Move Up       Tab - Switch between success/error       l - Log Screen        ESC - Main Menu",
    )).centered().bold()];

    if !results.fail_focus
        && screen == Some(Screen::RunningClean)
        && let Some((_, path)) = results
            .pos_succ
            .selected()
            .and_then(|i| report.success.get(i))
    {
        footer_lines.push(
            Line::from(format!("\nd - delete {:?}", path))
                .centered()
                .bold(),
        );
    }

    Paragraph::new(footer_lines)
        .block(Block::bordered())
        .render(footer, buf);
}

pub fn render_sort_options(
    buf: &mut Buffer,
    options: &SortOptions,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    Line::from("Sort Options")
        .bold()
        .centered()
        .render(title, buf);

    let body_split = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ]);
    let [path_area, num_area, _] = body_split.areas(body);
    let mut path_par = Paragraph::new(Line::from(options.path.as_str().bold()))
        .block(Block::bordered().title("Path"));

    if options.focus.selected() == Some(0) {
        match options.is_valid_path() {
            true => {
                path_par = path_par.block(
                    Block::bordered()
                        .title("Path")
                        .border_style(Style::new().green()),
                )
            }
            false => {
                path_par = path_par.block(
                    Block::bordered()
                        .title("Path")
                        .border_style(Style::new().red()),
                )
            }
        }
    }
    path_par.render(path_area, buf);

    let mut num_par = Paragraph::new(Line::from(options.num_show.as_str().bold()))
        .block(Block::bordered().title("Number of Files to show"));
    if options.focus.selected() == Some(1) {
        match options.num_show.as_str().parse::<usize>() {
            Ok(_) => {
                num_par = num_par.block(
                    Block::bordered()
                        .title("Number of Files to show")
                        .border_style(Style::new().green()),
                )
            }
            Err(_) => {
                num_par = num_par.block(
                    Block::bordered()
                        .title("Number of Files to show")
                        .border_style(Style::new().red()),
                )
            }
        }
    }
    num_par.render(num_area, buf);

    let mut footer_lines = vec![
        Line::from("ESC - Main Menu       Enter - Run       Tab/DownArrow - Down       BackTab/UpArrow - Up")
            .centered()
            .bold(),
    ];

    match &options.path_error {
        None => (),
        Some(e) => footer_lines.push(Line::from(e.as_str()).centered().red()),
    }

    match &options.num_error {
        None => (),
        Some(e) => footer_lines.push(Line::from(e.as_str()).centered().red()),
    }

    Paragraph::new(footer_lines)
        .block(Block::bordered())
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
    Line::from(title_str).centered().bold().render(title, buf);

    let list: List = logs.iter().map(|log| ListItem::new(log.as_str())).collect();
    let list = list
        .highlight_symbol(">>")
        .highlight_style(Style::new().reversed());
    let outer_block = Block::bordered().title("Logs");
    let inner = outer_block.inner(body);

    outer_block.render(body, buf);

    StatefulWidget::render(list, inner, buf, state);

    Paragraph::new(Line::from("q - quit       ESC - go back to result screen       J/DownArrow - Down       K/upArrow - Up").bold().centered())
            .block(Block::bordered())
            .render(footer, buf);
}
