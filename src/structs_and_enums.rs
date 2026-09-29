use crate::screens::*;
use ratatui::widgets::{ListState, Widget};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use std::io;
use std::sync::mpsc::{self, TryRecvError};

use crossterm::event;
use std::time::Duration;

use crate::key_handler::handle_key;
use crate::tools::CleanReport;
// The main struct.Hold info about the state of the entire TUI
pub struct App {
    pub exit: bool,
    pub menu: MenuState,
    pub clean: CleanState,
    pub sort: SortOptions,
    pub run_state: RunState,
    pub curr_screen: Screen,
    pub result_state: ResultStatus,
    pub log: LogState,
}

/// This struct saves the state of the current running clean job
#[derive(Default)]
pub struct RunState {
    pub rx: Option<Receiver<Message>>,
    pub status: Status,
    pub log_lines: Vec<String>,
    pub real_run: bool,
    pub remove_empty: bool,
    pub path: PathBuf,
    pub tick: u64,
}

// Menu items of the main menu
pub const MENU_ITEMS: [&str; 2] = ["Clean", "Sort"];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuState {
    pub list: ListState,
}

#[derive(Default)]
pub struct LogState {
    pub list: ListState,
}

// Holds the data about the clean screen
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanState {
    pub real_run: bool,
    pub remove_empty: bool,
    pub path: String,
    pub error: Option<String>,
    pub focus: Row,
}

// Holds the data about the sorting path and num of files to show
pub struct SortOptions {
    pub path: String,
    pub num_show: String,
    pub path_error: Option<String>,
    pub num_error: Option<String>,
    pub focus: ListState,
}

/// this enum holds the current state of the clean run
#[derive(Default)]
pub enum Status {
    #[default]
    Idle,
    Running,
    Finished,
    Failed(String),
}

#[derive(Default)]
pub struct ResultStatus {
    pub pos_succ: ListState,
    pub pos_fail: ListState,
    pub output: Option<CleanReport>,
    pub fail_focus: bool,
}

/// CurrentS screen selected
#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Main,
    CleanOptions,
    RunningClean,
    CleanResults,
    SortOptions,
    RunnignSort,
    Log,
}

/// Holds data that the sender sends from the cleaning job  
#[derive(Debug)]
pub enum Message {
    Log(String),
    Done(Result<CleanReport, String>),
}

// A very simple struct that holds info about the action of the user
// Only data is held when the user want to run. The data Will be gotten from the CleanState struct
#[derive(Debug)]
pub enum Action {
    Run {
        path: std::path::PathBuf,
        remove_empty: bool,
        real_run: bool,
    },
    Noop,
}

// All of the options of Clean options screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Path,
    Run,
}

impl MenuState {
    // Creating a new state and choosing the first option by default
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        MenuState { list: state }
    }
    // move to the next item
    pub fn next(&mut self) {
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
    pub fn prev(&mut self) {
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
    pub fn selected(&self) -> Option<usize> {
        self.list.selected()
    }
}

impl SortOptions {
    pub fn new(path: String) -> Self {
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

    pub fn push_char(&mut self, ch: char) {
        if self.focus.selected() == Some(0) {
            self.path.push(ch);
            self.validate_path();
        } else if self.focus.selected() == Some(1) {
            self.num_show.push(ch);
            self.validate_num();
        }
    }

    pub fn pop(&mut self) {
        if self.focus.selected() == Some(0) {
            self.path.pop();
            self.validate_path();
        } else if self.focus.selected() == Some(1) {
            self.num_show.pop();
            self.validate_num();
        }
    }

    pub fn validate_path(&mut self) {
        if !self.is_valid_path() {
            self.path_error = Some("Invalid Path".to_string());
        } else {
            self.path_error = None;
        }
    }

    pub fn change(&mut self) {
        if let Some(i) = self.focus.selected() {
            self.focus.select(Some(1 - i))
        }
    }

    pub fn valid_path(&mut self) -> Option<PathBuf> {
        if !self.is_valid_path() {
            return None;
        }
        Some(PathBuf::from(self.path.trim()))
    }

    pub fn validate_num(&mut self) {
        match self.num_show.as_str().parse::<usize>() {
            Ok(_) => self.num_error = None,
            Err(e) => self.num_error = Some(format!("Invalid Number {e}")),
        }
    }

    pub fn is_valid_path(&self) -> bool {
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
    pub fn new(path: String) -> Self {
        CleanState {
            real_run: false,
            remove_empty: false,
            path,
            focus: Row::Path,
            error: Some("Invalid Path".to_string()),
        }
    }
    // move to the next option
    pub fn next(&mut self) {
        let i = self.focus.index();

        if i >= Row::ALL.len() - 1 {
            self.focus = Row::from_index(0).unwrap_or(Row::Path);
        } else {
            self.focus = Row::from_index(i + 1).unwrap_or(Row::Path);
        }
    }
    // move to the previous option
    pub fn prev(&mut self) {
        let i = self.focus.index();

        if i == 0 {
            //wrap
            self.focus = Row::from_index(Row::ALL.len() - 1).unwrap_or(Row::Path);
        } else {
            self.focus = Row::from_index(i - 1).unwrap_or(Row::Path);
        }
    }
    // Maps row to a label (Becuase only toggles will show up in the list We only worry about them)
    pub fn label(&self, row: Row) -> String {
        match row {
            _ => String::from(""),
        }
    }
    // On each action we should mutate the state of the struct
    pub fn activate(&mut self) -> Action {
        self.error = None;
        match self.focus {
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
    pub fn push_char(&mut self, ch: char) {
        self.path.push(ch);
        if !self.is_valid_path() {
            self.error = Some("Provided path is not a valid directory".to_string());
        } else {
            self.error = None;
        }
    }
    pub fn backspace(&mut self) {
        self.path.pop();
        if !self.is_valid_path() {
            self.error = Some("Provided path is not a valid directory".to_string());
        } else {
            self.error = None;
        }
    }
    pub fn is_valid_path(&self) -> bool {
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
    const ALL: [Row; 2] = [Row::Path, Row::Run];
    pub fn from_index(i: usize) -> Option<Row> {
        Self::ALL.get(i).copied()
    }
    // maps row to name
    pub fn name(self) -> &'static str {
        match self {
            Row::Path => "Path",
            Row::Run => "Run",
        }
    }

    // maps row to hint (when hovering)
    pub fn hint(self) -> &'static str {
        match self {
            Row::Path => "Type to edit",
            Row::Run => "enter: start the scan",
        }
    }
    pub fn index(self) -> usize {
        Row::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }
}

impl App {
    // Runs the app
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            // Instead of wating for a keystorke (that might never come while cleaning is running)
            // we set a timeout of 100 milliseconds and drain after each pass
            if event::poll(Duration::from_millis(100))?
                && let crossterm::event::Event::Key(key) = crossterm::event::read()?
            {
                handle_key(self, key)?
            }
            self.drain();
        }

        Ok(())
    }
    // draws the screen
    pub fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
    // handdles key press

    /// drains the receiver end of the channel.
    pub fn drain(&mut self) {
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
            Screen::RunningClean => {
                render_running_clean("Running Clean", buf, &self.run_state, title, body, footer)
            }
            Screen::SortOptions => render_sort_options(buf, &self.sort, title, body, footer),
            Screen::RunnignSort => {
                render_running_clean("Running Sort", buf, &self.run_state, title, body, footer)
            }
            Screen::CleanResults => render_results_screen(
                "Clean results",
                buf,
                &mut self.result_state,
                &self.run_state.status,
                title,
                body,
                footer,
            ),
            Screen::Log => render_log(
                "Log Screen",
                buf,
                &self.run_state.log_lines,
                &mut self.log.list,
                title,
                body,
                footer,
            ),
        }
    }
}

impl ResultStatus {
    pub fn next(&mut self) {
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

    pub fn prev(&mut self) {
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
    pub fn reset_to_zero(&mut self) {
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

    pub fn move_to_end(&mut self) {
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

impl LogState {
    pub fn next(&mut self, len: usize) {
        if let Some(i) = self.list.selected() {
            if i >= len - 1 {
                self.list.select(Some(0));
            } else {
                self.list.select(Some(i + 1));
            }
        }
    }
    pub fn prev(&mut self, len: usize) {
        if let Some(i) = self.list.selected() {
            if i == 0 {
                self.list.select(Some(len - 1));
            } else {
                self.list.select(Some(i - 1));
            }
        }
    }

    pub fn pgdown(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.list.select(Some(len - 1));
    }

    pub fn pgup(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.list.select(Some(0));
    }
}
