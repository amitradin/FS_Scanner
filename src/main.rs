mod structs_and_enums;
use crate::structs_and_enums::{
    App, CleanState, LogState, MenuState, ResultStatus, RunState, Screen, SortOptions, WalkView,
};

mod key_handler;
mod screens;
mod theme;
mod tools;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    let mut app = App {
        exit: false,
        menu: MenuState::new(),
        clean: CleanState::default(),
        sort: SortOptions::default(),
        run_state: RunState::default(),
        curr_screen: Screen::Main,
        came_from: None,
        result_state: ResultStatus::default(),
        log: LogState::default(),
        walk: WalkView::default(),
    };
    let app_res = app.run(&mut terminal);
    ratatui::restore();
    Ok(app_res?)
}
