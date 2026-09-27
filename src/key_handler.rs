use crate::structs_and_enums::{
    Action, App, CleanState, LogState, Message, ResultStatus, Row, RunState, Screen, SortOptions,
    Status,
};
use crate::tools::{clean_main, sort_main};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::sync::mpsc;

///Key Hanlding logic
pub fn handle_key(app: &mut App, key: KeyEvent) -> std::io::Result<()> {
    if key.kind == KeyEventKind::Press {
        match app.curr_screen {
            Screen::Main => {
                if key.code == KeyCode::Char('q') {
                    app.exit = true;
                } else if key.code == KeyCode::Char('j')
                    || key.code == KeyCode::Tab
                    || key.code == KeyCode::Down
                {
                    app.menu.next();
                } else if key.code == KeyCode::Char('k')
                    || key.code == KeyCode::BackTab
                    || key.code == KeyCode::Up
                {
                    app.menu.prev();
                }
                if key.code == KeyCode::Enter {
                    match app.menu.selected() {
                        Some(0) => app.curr_screen = Screen::CleanOptions,
                        Some(1) => app.curr_screen = Screen::SortOptions,
                        _ => (),
                    }
                }
            }

            Screen::SortOptions => {
                if key.code == KeyCode::Enter {
                    let path = app.sort.valid_path();
                    let num = app.sort.num_show.as_str().parse::<usize>();
                    match path {
                        Some(p) if num.is_ok() => {
                            let (sender, receiver) = mpsc::channel::<Message>();
                            app.run_state = RunState {
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
                            app.curr_screen = Screen::RunnignSort;
                        }
                        _ => (),
                    }
                }
                if key.code == KeyCode::Esc {
                    app.curr_screen = Screen::Main;
                } else if key.code == KeyCode::Tab
                    || key.code == KeyCode::BackTab
                    || key.code == KeyCode::Down
                    || key.code == KeyCode::Up
                {
                    app.sort.change();
                } else {
                    match key.code {
                        KeyCode::Char(c)
                            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                        {
                            app.sort.push_char(c)
                        }
                        KeyCode::Backspace => app.sort.pop(),
                        _ => (),
                    }
                }
            }

            Screen::CleanOptions => {
                if key.code == KeyCode::Esc {
                    app.curr_screen = Screen::Main;
                    app.clean.focus = Row::RealRun;
                    return Ok(());
                } else if key.code == KeyCode::Down || key.code == KeyCode::Tab {
                    app.clean.next();
                    return Ok(());
                } else if key.code == KeyCode::Up || key.code == KeyCode::BackTab {
                    app.clean.prev();
                    return Ok(());
                }

                if app.clean.focus == Row::Path {
                    match key.code {
                        KeyCode::Char(c) => {
                            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                                app.clean.push_char(c)
                            }
                        }
                        KeyCode::Backspace => app.clean.backspace(),
                        KeyCode::Enter => app.clean.next(),
                        _ => (),
                    }
                } else {
                    if key.code == KeyCode::Char('j') {
                        app.clean.next();
                    }
                    if key.code == KeyCode::Char('k') {
                        app.clean.prev();
                    }
                    if key.code == KeyCode::Char(' ') {
                        if app.clean.focus.is_toggle() {
                            app.clean.activate();
                        }
                    } else if key.code == KeyCode::Enter && app.clean.focus == Row::Run {
                        let action = app.clean.activate();
                        match action {
                            Action::Noop => (),
                            Action::Run {
                                path,
                                remove_empty,
                                real_run,
                            } => {
                                let (sender, receiver) = mpsc::channel::<Message>();
                                app.run_state = RunState {
                                    rx: Some(receiver),
                                    status: Status::Running,
                                    log_lines: Vec::new(),
                                    real_run,
                                    remove_empty,
                                    path: path.clone(),
                                    tick: 0,
                                };

                                std::thread::spawn(move || {
                                    let res =
                                        clean_main(&path, remove_empty, real_run, sender.clone());
                                    let _ = sender.send(Message::Done(res));
                                });
                                app.curr_screen = Screen::RunningClean;
                            }
                        }
                    }
                }
            }
            Screen::RunningClean => {
                if key.code == KeyCode::Char('q') {
                    app.exit = true;
                }
            }
            Screen::RunnignSort => {
                if key.code == KeyCode::Char('q') {
                    app.exit = true;
                }
            }
            //On Esc we should reset the app
            Screen::CleanResults => {
                if key.code == KeyCode::Esc {
                    app.curr_screen = Screen::Main;
                    app.clean = CleanState::new(String::from(""));
                    app.sort = SortOptions::new(String::from(""));
                    app.run_state = RunState::default();
                    app.result_state = ResultStatus::default();
                    app.log = LogState::default();
                }
                if key.code == KeyCode::Char('q') {
                    app.exit = true;
                }
                if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
                    app.result_state.next();
                }
                if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
                    app.result_state.prev();
                }
                if key.code == KeyCode::Tab {
                    app.result_state.fail_focus = !app.result_state.fail_focus;
                }
                if key.code == KeyCode::PageUp {
                    app.result_state.reset_to_zero();
                }
                if key.code == KeyCode::PageDown {
                    app.result_state.move_to_end();
                }
                if key.code == KeyCode::Char('l') {
                    app.curr_screen = Screen::Log;
                    if !app.run_state.log_lines.is_empty() {
                        app.log.list.select(Some(0));
                    }
                }
            }
            Screen::Log => {
                let len = app.run_state.log_lines.len();
                if key.code == KeyCode::Char('q') {
                    app.exit = true;
                }
                if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
                    app.log.next(len);
                }

                if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
                    app.log.prev(len);
                }

                if key.code == KeyCode::Esc {
                    app.curr_screen = Screen::CleanResults
                }

                if key.code == KeyCode::PageUp {
                    app.log.pgup(len);
                }
                if key.code == KeyCode::PageDown {
                    app.log.pgdown(len);
                }
            }
        }
    }
    Ok(())
}
