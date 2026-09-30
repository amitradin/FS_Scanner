use crate::structs_and_enums::Screen::RunningClean;
use crate::structs_and_enums::{
    Action, App, CleanState, LogState, Message, ResultStatus, Row, RunState, Screen, SortOptions,
    Status,
};
use crate::tools::{clean_main, delete_file, sort_main};
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
                            app.came_from = Some(Screen::RunnignSort);
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
                    app.clean.focus = Row::Path;
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
                                    let res = clean_main(&path, sender.clone());
                                    let _ = sender.send(Message::Done(res));
                                });
                                app.came_from = Some(Screen::RunningClean);
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
                    app.came_from = None;
                }
                if key.code == KeyCode::Char('q') {
                    app.exit = true;
                }
                if !app.result_state.is_popup {
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

                    if key.code == KeyCode::Char('d')
                        && !app.result_state.fail_focus
                        && app.came_from == Some(RunningClean)
                    {
                        app.result_state.is_popup = !app.result_state.is_popup;
                    }
                } else {
                    if key.code == KeyCode::Char('y') {
                        let curr_index = app.result_state.pos_succ.selected();
                        if curr_index.is_none() {
                            app.result_state.is_popup = !app.result_state.is_popup;
                            return Ok(());
                        }
                        let curr_index = curr_index.unwrap();
                        let report = app.result_state.output.as_mut();
                        if report.is_none() {
                            app.result_state.is_popup = !app.result_state.is_popup;
                            return Ok(());
                        }

                        if let Some(valid_report) = report {
                            let path = &valid_report.success[curr_index].1;
                            if let Ok(()) = delete_file(path) {
                                if curr_index == valid_report.success.len() - 1 {
                                    if curr_index > 0 {
                                        app.result_state.pos_succ.select(Some(curr_index - 1));
                                    } else {
                                        app.result_state.pos_succ.select(None);
                                    }
                                }
                                valid_report.success.remove(curr_index);
                            }
                        }

                        app.result_state.is_popup = !app.result_state.is_popup
                    }
                    if key.code == KeyCode::Char('n') {
                        app.result_state.is_popup = !app.result_state.is_popup;
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
