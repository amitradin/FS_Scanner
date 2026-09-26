mod tools;
use crate::tools::{clean_main, sort_main};
use tools::CleanReport;

use std::path::PathBuf;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;
use std::{io, sync::mpsc::Receiver};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, List, ListItem, ListState, Paragraph, StatefulWidget, Widget},
};

use ratatui_spinner::LinearSpinner;

// The main struct.Hold info about the state of the entire TUI
struct App {
    exit: bool,
    menu: MenuState,
    clean: CleanState,
    sort: SortOptions,
    run_state: RunState,
    curr_screen: Screen,
    result_state: ResultStatus,
}

/// This struct saves the state of the current running clean job
#[derive(Default)]
struct RunState {
    rx: Option<Receiver<Message>>,
    status: Status,
    log_lines: Vec<String>,
    real_run: bool,
    remove_empty: bool,
    path: PathBuf,
    tick: u64,
}

// Menu items of the main menu
const MENU_ITEMS: [&str; 2] = ["Clean", "Sort"];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MenuState {
    list: ListState,
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

// Holds the data about the sorting path and num of files to show
struct SortOptions {
    path: String,
    num_show: String,
    path_error: Option<String>,
    num_error: Option<String>,
    focus: ListState,
}

/// this enum holds the current state of the clean run
#[derive(Default)]
enum Status {
    #[default]
    Idle,
    Running,
    Finished,
    Failed(String),
}

#[derive(Default)]
struct ResultStatus {
    pos_succ: ListState,
    pos_fail: ListState,
    output: Option<CleanReport>,
    fail_focus: bool,
}

/// CurrentS screen selected
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    CleanOptions,
    RunningClean,
    CleanResults,
    SortOptions,
    RunnignSort,
}

/// Holds data that the sender sends from the cleaning job  
#[derive(Debug)]
enum Message {
    Log(String),
    Done(Result<CleanReport, String>),
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

impl SortOptions {
    fn new(path: String) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        SortOptions {
            path,
            num_show: "20".to_string(),
            path_error: Some("Invalid path".to_string()),
            num_error: None,
            focus: state,
        }
    }

    fn push_char(&mut self, ch: char) {
        if self.focus.selected() == Some(0) {
            self.path.push(ch);
            self.validate_path();
        } else if self.focus.selected() == Some(1) {
            self.num_show.push(ch);
            self.validate_num();
        }
    }

    fn pop(&mut self) {
        if self.focus.selected() == Some(0) {
            self.path.pop();
            self.validate_path();
        } else if self.focus.selected() == Some(1) {
            self.num_show.pop();
            self.validate_num();
        }
    }

    fn validate_path(&mut self) {
        if !self.is_valid_path() {
            self.path_error = Some("Invalid Path".to_string());
        } else {
            self.path_error = None;
        }
    }

    fn change(&mut self) {
        if let Some(i) = self.focus.selected() {
            self.focus.select(Some(1 - i))
        }
    }

    fn valid_path(&mut self) -> Option<PathBuf> {
        if !self.is_valid_path() {
            return None;
        }
        Some(PathBuf::from(self.path.trim()))
    }

    fn validate_num(&mut self) {
        match self.num_show.as_str().parse::<usize>() {
            Ok(_) => self.num_error = None,
            Err(e) => self.num_error = Some(format!("Invalid Number {e}")),
        }
    }

    fn is_valid_path(&self) -> bool {
        let trimmed = &self.path.trim();
        if trimmed.is_empty() {
            return false;
        }
        let path = PathBuf::from(trimmed);
        path.is_dir()
    }
}

impl CleanState {
    // a default method. gets a path.
    fn new(path: String) -> Self {
        CleanState {
            real_run: false,
            remove_empty: false,
            path,
            focus: Row::RealRun,
            error: Some("Invalid Path".to_string()),
        }
    }
    // move to the next option
    fn next(&mut self) {
        let i = self.focus.index();

        if i >= Row::ALL.len() - 1 {
            self.focus = Row::from_index(0).unwrap_or(Row::RealRun);
        } else {
            self.focus = Row::from_index(i + 1).unwrap_or(Row::RealRun);
        }
    }
    // move to the previous option
    fn prev(&mut self) {
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
        if !self.is_valid_path() {
            self.error = Some("Provided path is not a valid directory".to_string());
        } else {
            self.error = None;
        }
    }
    fn backspace(&mut self) {
        self.path.pop();
        if !self.is_valid_path() {
            self.error = Some("Provided path is not a valid directory".to_string());
        } else {
            self.error = None;
        }
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
        matches!(self, Row::RealRun | Row::RemoveEmpty)
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

impl App {
    // Runs the app
    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            // Instead of wating for a keystorke (that might never come while cleaning is running)
            // we set a timeout of 100 milliseconds and drain after each pass
            if event::poll(Duration::from_millis(100))?
                && let crossterm::event::Event::Key(key) = crossterm::event::read()?
            {
                self.handle_key(key)?
            }
            self.drain();
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
                            Some(1) => self.curr_screen = Screen::SortOptions,
                            _ => (),
                        }
                    }
                }

                Screen::SortOptions => {
                    if key.code == KeyCode::Enter {
                        let path = self.sort.valid_path();
                        let num = self.sort.num_show.as_str().parse::<usize>();
                        match path {
                            Some(p) if num.is_ok() => {
                                let (sender, receiver) = mpsc::channel::<Message>();
                                self.run_state = RunState {
                                    rx: Some(receiver),
                                    status: Status::Running,
                                    log_lines: Vec::new(),
                                    real_run: false,
                                    remove_empty: false,
                                    path: p.clone(),
                                    tick: 0,
                                };
                                std::thread::spawn(move || {
                                    let res = sort_main(&p, num.unwrap_or(20), sender.clone());
                                    let _ = sender.send(Message::Done(res));
                                });
                                self.curr_screen = Screen::RunnignSort;
                            }
                            _ => (),
                        }
                    }
                    if key.code == KeyCode::Esc {
                        self.curr_screen = Screen::Main;
                    } else if key.code == KeyCode::Tab
                        || key.code == KeyCode::BackTab
                        || key.code == KeyCode::Down
                        || key.code == KeyCode::Up
                    {
                        self.sort.change();
                    } else {
                        match key.code {
                            KeyCode::Char(c)
                                if key.modifiers.is_empty()
                                    || key.modifiers == KeyModifiers::SHIFT =>
                            {
                                self.sort.push_char(c)
                            }
                            KeyCode::Backspace => self.sort.pop(),
                            _ => (),
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
                            KeyCode::Char(c) => {
                                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT
                                {
                                    self.clean.push_char(c)
                                }
                            }
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
                        } else if key.code == KeyCode::Enter && self.clean.focus == Row::Run {
                            let action = self.clean.activate();
                            match action {
                                Action::Noop => (),
                                Action::Run {
                                    path,
                                    remove_empty,
                                    real_run,
                                } => {
                                    let (sender, receiver) = mpsc::channel::<Message>();
                                    self.run_state = RunState {
                                        rx: Some(receiver),
                                        status: Status::Running,
                                        log_lines: Vec::new(),
                                        real_run,
                                        remove_empty,
                                        path: path.clone(),
                                        tick: 0,
                                    };

                                    std::thread::spawn(move || {
                                        let res = clean_main(
                                            &path,
                                            remove_empty,
                                            real_run,
                                            sender.clone(),
                                        );
                                        let _ = sender.send(Message::Done(res));
                                    });
                                    self.curr_screen = Screen::RunningClean;
                                }
                            }
                        }
                    }
                }
                Screen::RunningClean => {
                    if key.code == KeyCode::Char('q') {
                        self.exit = true;
                    }
                }
                Screen::RunnignSort => {
                    if key.code == KeyCode::Char('q') {
                        self.exit = true;
                    }
                }
                Screen::CleanResults => {
                    if key.code == KeyCode::Char('q') {
                        self.exit = true;
                    }
                    if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
                        self.result_state.next();
                    }
                    if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
                        self.result_state.prev();
                    }
                    if key.code == KeyCode::Tab {
                        self.result_state.fail_focus = !self.result_state.fail_focus;
                    }
                    if key.code == KeyCode::PageUp {
                        self.result_state.reset_to_zero();
                    }
                    if key.code == KeyCode::PageDown {
                        self.result_state.move_to_end();
                    }
                }
            }
        }
        Ok(())
    }

    /// drains the receiver end of the channel.
    fn drain(&mut self) {
        // next tick for the animation
        self.run_state.tick += 1;
        let mut message = self.run_state.rx.take();
        if message.is_some() {
            // trying to read from receiver until it is empty, it might not have something at all,
            // thats why we are inside the if
            loop {
                let res = message.as_ref().unwrap().try_recv();
                match res {
                    Ok(m) => match m {
                        Message::Log(s) => {
                            self.run_state.log_lines.push(s);
                        }
                        Message::Done(Ok(report)) => {
                            if !report.success.is_empty() {
                                self.result_state.pos_succ.select(Some(0));
                            } else {
                                self.result_state.pos_succ.select(None);
                            }

                            if !report.errors.is_empty() {
                                self.result_state.pos_fail.select(Some(0));
                            } else {
                                self.result_state.pos_fail.select(None);
                            }
                            self.result_state.fail_focus = false;

                            self.result_state.output = Some(report);
                            self.run_state.status = Status::Finished;
                            self.curr_screen = Screen::CleanResults;

                            return;
                        }
                        Message::Done(Err(e)) => {
                            self.run_state.status = Status::Failed(e);
                            self.curr_screen = Screen::CleanResults;
                            self.result_state.pos_succ.select(Some(0));
                            self.result_state.pos_fail.select(Some(0));
                            self.result_state.fail_focus = false;
                            return;
                        }
                    },
                    Err(TryRecvError::Empty) => {
                        self.run_state.rx = message.take();
                        return;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.run_state.status = Status::Failed("Worker crashed".into());
                        self.run_state.rx = None;
                        self.curr_screen = Screen::CleanResults;
                        return;
                    }
                }
            }
        }
    }
}

/// We must implement render for this trait
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let vertical_layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(5),
        ]);

        let [title, body, footer] = vertical_layout.areas(area);

        match self.curr_screen {
            Screen::Main => render_menu(buf, &mut self.menu.list, title, body, footer),
            Screen::CleanOptions => render_clean(buf, &mut self.clean, title, body, footer),
            Screen::RunningClean => render_running_clean(
                "Running Clean",
                Screen::RunningClean,
                buf,
                &self.run_state,
                title,
                body,
                footer,
            ),
            Screen::SortOptions => render_sort_options(buf, &self.sort, title, body, footer),
            Screen::RunnignSort => render_running_clean(
                "Running Sort",
                Screen::RunnignSort,
                buf,
                &self.run_state,
                title,
                body,
                footer,
            ),
            Screen::CleanResults => render_results_screen(
                "Clean results",
                buf,
                &mut self.result_state,
                &self.run_state.status,
                title,
                body,
                footer,
            ),
        }
    }
}

impl ResultStatus {
    fn next(&mut self) {
        if self.output.is_none() {
            return;
        }
        match self.fail_focus {
            false if !self.output.as_ref().unwrap().success.is_empty() => {
                let curr = self.pos_succ.selected().unwrap_or(0);
                if curr >= self.output.as_ref().unwrap().success.len() - 1 {
                    self.pos_succ.select(Some(0));
                } else {
                    self.pos_succ.select(Some(curr + 1));
                }
            }
            true if !self.output.as_ref().unwrap().errors.is_empty() => {
                let curr = self.pos_fail.selected().unwrap_or(0);
                if curr >= self.output.as_ref().unwrap().errors.len() - 1 {
                    self.pos_fail.select(Some(0));
                } else {
                    self.pos_fail.select(Some(curr + 1));
                }
            }
            _ => (),
        }
    }

    fn prev(&mut self) {
        if self.output.is_none() {
            return;
        }
        match self.fail_focus {
            false if !self.output.as_ref().unwrap().success.is_empty() => {
                let curr = self.pos_succ.selected().unwrap_or(0);
                if curr == 0 {
                    self.pos_succ
                        .select(Some(self.output.as_ref().unwrap().success.len() - 1));
                } else {
                    self.pos_succ.select(Some(curr - 1));
                }
            }
            true if !self.output.as_ref().unwrap().errors.is_empty() => {
                let curr = self.pos_fail.selected().unwrap_or(0);
                if curr == 0 {
                    self.pos_fail
                        .select(Some(self.output.as_ref().unwrap().errors.len() - 1));
                } else {
                    self.pos_fail.select(Some(curr - 1));
                }
            }
            _ => (),
        }
    }
    fn reset_to_zero(&mut self) {
        if self.output.is_none() {
            return;
        }
        match self.fail_focus {
            false if !self.output.as_ref().unwrap().success.is_empty() => {
                self.pos_succ.select(Some(0))
            }
            true if !self.output.as_ref().unwrap().errors.is_empty() => {
                self.pos_fail.select(Some(0))
            }
            _ => (),
        }
    }

    fn move_to_end(&mut self) {
        if self.output.is_none() {
            return;
        }
        match self.fail_focus {
            false if !self.output.as_ref().unwrap().success.is_empty() => self
                .pos_succ
                .select(Some(self.output.as_ref().unwrap().success.len() - 1)),
            true if !self.output.as_ref().unwrap().errors.is_empty() => self
                .pos_fail
                .select(Some(self.output.as_ref().unwrap().errors.len() - 1)),
            _ => (),
        }
    }
}

/// rendering the main screen function
fn render_menu(buf: &mut Buffer, state: &mut ListState, title: Rect, body: Rect, footer: Rect) {
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

    let second_line = Line::from(options.focus.hint()).centered().bold();
    text.push(line);
    text.push(second_line);

    if let Some(e) = &options.error {
        text.push(Line::from(e.as_str().red()).centered())
    };

    Paragraph::new(text).render(inner_footer, buf);
}

// Loading screen when waiting for clean to finish
fn render_running_clean(
    title_str: &str,
    screen: Screen,
    buf: &mut Buffer,
    options: &RunState,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    Line::from(title_str).bold().centered().render(title, buf);

    let spinner = LinearSpinner::new(options.tick).total_slots(10);

    let body_split = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(4),
        Constraint::Min(0),
    ]);
    let [loading_area, flags_area, log_area] = body_split.areas(body);
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

    let mut flags = Vec::new();
    if screen == Screen::RunningClean {
        let real_run = match options.real_run {
            true => Line::from("WARNING - Deleting files").red(),
            false => Line::from("Dry run - not deleting files").green(),
        };

        let remove_empty = match options.remove_empty {
            true => Line::from("Remove empty files: Yes"),
            false => Line::from("Remove empty files: No"),
        };
        flags.append(&mut vec![real_run, remove_empty]);
        Paragraph::new(flags)
            .block(Block::bordered().title("Flags"))
            .render(flags_area, buf);
    }

    // To render the logs, we take the last n lines of the log. We use log_area.height to use the
    // current height as an indicator of how many lines we can render. we use -2 since the area is
    // also surrounded by a border
    let last_items = options
        .log_lines
        .iter()
        .rev()
        .take((log_area.height - 2) as usize)
        .rev()
        .map(|log| ListItem::new(log.as_str()))
        .collect::<Vec<ListItem>>();

    let log_block = Block::bordered().title("Logs");
    let inner_log = log_block.inner(log_area);
    log_block.render(log_area, buf);

    Widget::render(List::new(last_items), inner_log, buf);

    Paragraph::new(Line::from("q - quit").bold().centered())
        .block(Block::bordered())
        .render(footer, buf);
}

fn render_results_screen(
    title_str: &str,
    buf: &mut Buffer,
    results: &mut ResultStatus,
    state: &Status,
    title: Rect,
    body: Rect,
    footer: Rect,
) {
    Line::from(title_str).bold().centered().render(title, buf);

    if results.output.is_none() {
        return;
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

    let report = results.output.as_ref().unwrap();

    let succ_items = report
        .success
        .iter()
        .map(|item| ListItem::new(item.as_str()))
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

    Paragraph::new(Line::from(
        "q - quit       J/DownArrow - Move Down       K/UpArrow - Move Up       Tab - Switch between success/error",
    ).bold().centered()).block(Block::bordered()).render(footer, buf);
}

fn render_sort_options(
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
    let mut path_par =
        Paragraph::new(Line::from(options.path.as_str().bold())).block(Block::bordered());

    if options.focus.selected() == Some(0) {
        match options.is_valid_path() {
            true => path_par = path_par.block(Block::bordered().border_style(Style::new().green())),
            false => path_par = path_par.block(Block::bordered().border_style(Style::new().red())),
        }
    }
    path_par.render(path_area, buf);

    let mut num_par =
        Paragraph::new(Line::from(options.num_show.as_str().bold())).block(Block::bordered());
    if options.focus.selected() == Some(1) {
        match options.num_show.as_str().parse::<usize>() {
            Ok(_) => num_par = num_par.block(Block::bordered().border_style(Style::new().green())),
            Err(_) => num_par = num_par.block(Block::bordered().border_style(Style::new().red())),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    let mut app = App {
        exit: false,
        menu: MenuState::new(),
        clean: CleanState::new(String::from("")),
        sort: SortOptions::new(String::from("")),
        run_state: RunState::default(),
        curr_screen: Screen::Main,
        result_state: ResultStatus::default(),
    };
    let app_res = app.run(&mut terminal);
    ratatui::restore();
    Ok(app_res?)
}
