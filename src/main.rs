mod structs_and_enums;
use crate::structs_and_enums::App;

mod key_handler;
mod screens;
mod theme;
mod tools;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    let app_res = app.run(&mut terminal);
    ratatui::restore();
    Ok(app_res?)
}
