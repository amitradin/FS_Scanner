mod tools;

use std::io;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, List, ListItem, ListState, Paragraph, StatefulWidget, Widget},
};

/*
useclap::{Parser, error::Result};
use tools::Options;
use tools::{populate_paths, run_clean, run_sort}; */

// The main struct.Hold info about the state of the entire TUI
struct App {
    exit: bool,
    menu: MenuState,
    clean: CleanState,
    curr_screen: Screen,
}
// Current screen selected
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    CleanOptions,
}

// Menu items of the main menu
const MENU_ITEMS: [&str; 2] = ["Clean", "Sort"];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MenuState {
    list: ListState,
}
impl MenuState {
    // Creating a new state and choosing the first option by default
    fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        MenuState { list: state }
    }
    // move to the next item
    fn next(&mut self) {
        let i = match self.list.selected() {
            Some(i) => {
                if i >= MENU_ITEMS.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list.select(Some(i));
    }
    // move to the previous item
    fn prev(&mut self) {
        let i = match self.list.selected() {
            Some(i) => {
                if i == 0 {
                    MENU_ITEMS.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list.select(Some(i));
    }
    // gets the selected option (index)
    fn selected(&self) -> Option<usize> {
        self.list.selected()
    }
}

// Holds the data about the clean screen
#[derive(Clone, Debug, PartialEq, Eq)]
struct CleanState {
    real_run: bool,
    remove_empty: bool,
    path: String,
    error: Option<String>,
    focus: Row,
}
impl CleanState {
    // a default method. gets a path.
    fn new(path: String) -> Self {
        CleanState {
            real_run: false,
            remove_empty: false,
            path: path,
            focus: Row::RealRun,
            error: None,
        }
    }
    // move to the next option
    fn next(&mut self) {
        self.error = None;
        let i = self.focus.index();

        if i >= Row::ALL.len() - 1 {
            self.focus = Row::from_index(0).unwrap_or(Row::RealRun);
        } else {
            self.focus = Row::from_index(i + 1).unwrap_or(Row::RealRun);
        }
    }
    // move to the previous option
    fn prev(&mut self) {
        self.error = None;
        let i = self.focus.index();

        if i == 0 {
            //wrap
            self.focus = Row::from_index(Row::ALL.len() - 1).unwrap_or(Row::RealRun);
        } else {
            self.focus = Row::from_index(i - 1).unwrap_or(Row::RealRun);
        }
    }
    // Maps row to a label (Becuase only toggles will show up in the list We only worry about them)
    fn label(&self, row: Row) -> String {
        match row {
            Row::RealRun => match self.real_run {
                true => format!("[x] {}", row.name()),
                false => format!("[ ] {}", row.name()),
            },
            Row::RemoveEmpty => match self.remove_empty {
                true => format!("[x] {}", row.name()),
                false => format!("[ ] {}", row.name()),
            },
            _ => String::from(""),
        }
    }
    // On each action we should mutate the state of the struct
    fn activate(&mut self) -> Action {
        self.error = None;
        match self.focus {
            Row::RealRun => {
                self.real_run = !self.real_run;
                Action::Noop
            }
            Row::RemoveEmpty => {
                self.remove_empty = !self.remove_empty;
                Action::Noop
            }
            Row::Path => {
                self.focus = Row::Run;
                Action::Noop
            }
            Row::Run => {
                let trimmed = &self.path.trim();
                if trimmed.is_empty() {
                    self.error = Some(String::from("Path does not exist"));
                    return Action::Noop;
                }
                let path = PathBuf::from(trimmed);
                if path.is_dir() {
                    Action::Run {
                        path,
                        remove_empty: self.remove_empty,
                        real_run: self.real_run,
                    }
                } else {
                    self.error = Some(String::from("Provided path is not a valid directory"));
                    Action::Noop
                }
            }
        }
    }
    fn push_char(&mut self, ch: char) {
        self.path.push(ch);
    }
    fn backspace(&mut self) {
        self.path.pop();
    }
    fn is_valid_path(&self) -> bool {
        let trimmed = &self.path.trim();
        if trimmed.is_empty() {
            return false;
        }
        if !PathBuf::from(trimmed).is_dir() {
            return false;
        }
        true
    }
}

// A very simple struct that holds info about the action of the user
// Only data is held when the user want to run. The data Will be gotten from the CleanState struct
#[derive(Debug)]
enum Action {
    Run {
        path: std::path::PathBuf,
        remove_empty: bool,
        real_run: bool,
    },
    Noop,
}

// All of the options of Clean options screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    RealRun,
    RemoveEmpty,
    Path,
    Run,
}

impl Row {
    // All of the row options such that it would be easier to index into
    const ALL: [Row; 4] = [Row::RealRun, Row::RemoveEmpty, Row::Path, Row::Run];
    fn from_index(i: usize) -> Option<Row> {
        Self::ALL.get(i).copied()
    }
    // maps row to name
    fn name(self) -> &'static str {
        match self {
            Row::RealRun => "Real run (Will delete items)",
            Row::RemoveEmpty => "Remove empty files",
            Row::Path => "Path",
            Row::Run => "Run",
        }
    }
    fn is_toggle(self) -> bool {
        match self {
            Row::RealRun => true,
            Row::RemoveEmpty => true,
            _ => false,
        }
    }
    // maps row to hint (when hovering)
    fn hint(self) -> &'static str {
        match self {
            Row::RealRun => "space: toggle - WARNING: (deletes files for real)",
            Row::RemoveEmpty => "space: toggle- also delete zero-byte files",
            Row::Path => "Type to edit",
            Row::Run => "enter: start the scan",
        }
    }
    fn index(self) -> usize {
        Row::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    /*let op = Options::parse();
    let mut files = populate_paths(&op.path, op.remove_empty_files, op.clean)?;
    if op.clean {
        run_clean(files, &op)?;
    } else {
        run_sort(&mut files, op.num_sorting);
    } */
    let mut terminal = ratatui::init();
    let mut app = App {
        exit: false,
        menu: MenuState::new(),
        clean: CleanState::new(String::from("")),
        curr_screen: Screen::Main,
    };
    let app_res = app.run(&mut terminal);
    ratatui::restore();
    Ok(app_res?)
}

impl App {
    // Runs the app
    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            match crossterm::event::read()? {
                crossterm::event::Event::Key(key) => self.handle_key(key)?,
                _ => {}
            }
        }
        Ok(())
    }
    // draws the screen
    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
    // handdles key press
    fn handle_key(&mut self, key: KeyEvent) -> io::Result<()> {
        if key.kind == KeyEventKind::Press {
            match self.curr_screen {
                Screen::Main => {
                    if key.code == KeyCode::Char('q') {
                        self.exit = true;
                    } else if key.code == KeyCode::Char('j')
                        || key.code == KeyCode::Tab
                        || key.code == KeyCode::Down
                    {
                        self.menu.next();
                    } else if key.code == KeyCode::Char('k')
                        || key.code == KeyCode::BackTab
                        || key.code == KeyCode::Up
                    {
                        self.menu.prev();
                    }
                    if key.code == KeyCode::Enter {
                        match self.menu.selected() {
                            Some(0) => self.curr_screen = Screen::CleanOptions,
                            _ => {}
                        }
                    }
                }

                Screen::CleanOptions => {
                    if key.code == KeyCode::Esc {
                        self.curr_screen = Screen::Main;
                        self.clean.focus = Row::RealRun;
                        return Ok(());
                    } else if key.code == KeyCode::Down || key.code == KeyCode::Tab {
                        self.clean.next();
                        return Ok(());
                    } else if key.code == KeyCode::Up || key.code == KeyCode::BackTab {
                        self.clean.prev();
                        return Ok(());
                    }

                    if self.clean.focus == Row::Path {
                        match key.code {
                            KeyCode::Char(c) if key.modifiers.is_empty() => self.clean.push_char(c),
                            KeyCode::Backspace => self.clean.backspace(),
                            KeyCode::Enter => self.clean.next(),
                            _ => (),
                        }
                    } else {
                        if key.code == KeyCode::Char('j') {
                            self.clean.next();
                        }
                        if key.code == KeyCode::Char('k') {
                            self.clean.prev();
                        }
                        if key.code == KeyCode::Char(' ') {
                            if self.clean.focus.is_toggle() {
                                self.clean.activate();
                            }
                        } else if key.code == KeyCode::Enter {
                            match self.clean.focus {
                                Row::Run => {
                                    let action = self.clean.activate();
                                    match action {
                                        Action::Noop => (),
                                        Action::Run { .. } => (),
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let vertical_layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(4),
        ]);

        let [title, body, footer] = vertical_layout.areas(area);

        match self.curr_screen {
            Screen::Main => render_menu(buf, &mut self.menu.list, title, body, footer),
            Screen::CleanOptions => render_clean(buf, &mut self.clean, title, body, footer),
        }
    }
}

fn render_menu(buf: &mut Buffer, state: &mut ListState, title: Rect, body: Rect, footer: Rect) {
    let items = MENU_ITEMS
        .into_iter()
        .map(|i| ListItem::new(i))
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
fn render_clean(buf: &mut Buffer, options: &mut CleanState, title: Rect, body: Rect, footer: Rect) {
    Line::from("Clean Options")
        .bold()
        .centered()
        .render(title, buf);

    let vertical_body = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ]);

    let [toggles, path, run, _] = vertical_body.areas(body);
    let mut lines = Vec::new();
    match options.focus {
        Row::RealRun => {
            lines.push(
                Line::from(format!(">> {}", options.label(Row::RealRun)))
                    .bold()
                    .reversed(),
            );
            lines.push(Line::from(format!("   {}", options.label(Row::RemoveEmpty))).bold());
        }
        Row::RemoveEmpty => {
            lines.push(Line::from(format!("   {}", options.label(Row::RealRun))).bold());
            lines.push(
                Line::from(format!(">> {}", options.label(Row::RemoveEmpty)))
                    .reversed()
                    .bold(),
            );
        }
        _ => {
            lines.push(Line::from(format!("   {}", options.label(Row::RealRun))).bold());
            lines.push(Line::from(format!("   {}", options.label(Row::RemoveEmpty))).bold());
        }
    }
    let mut toggle_block = Block::bordered().title("Toggles");
    if options.focus.is_toggle() {
        toggle_block = toggle_block.border_style(Style::new().green());
    }
    let inner_toggle = toggle_block.inner(toggles);
    toggle_block.render(toggles, buf);
    Paragraph::new(lines).render(inner_toggle, buf);

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
    text.push(line);
    let mut second_line = match &options.error {
        Some(e) => Line::from(e.as_str().red()),
        None => Line::from(options.focus.hint()),
    };
    second_line = second_line.centered().bold();
    text.push(second_line);

    Paragraph::new(text).render(inner_footer, buf);
}
