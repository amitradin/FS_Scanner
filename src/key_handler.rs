use crate::structs_and_enums::{
    Action, App, Message, Row, RunState, Screen, Status, Walk, WalkView,
};
use crate::tools::{clean_main, delete_file, sort_main};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::sync::mpsc;
use std::time::Instant;

///Key Hanlding logic
pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.kind == KeyEventKind::Press {
        match app.curr_screen {
            Screen::Main => handle_main(app, key),

            Screen::SortOptions => handle_sort_options(app, key),

            Screen::CleanOptions => handle_clean_options(app, key),

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
            Screen::CleanResults => handle_clean_results(app, key),
            Screen::Log => handle_log(app, key),
            Screen::Walk => handle_walk(app, key),
            Screen::Loading => {
                if key.code == KeyCode::Char('q') {
                    app.exit = true;
                }
            }
        }
    }
}

fn handle_main(app: &mut App, key: KeyEvent) {
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
            Some(2) => {
                let (sender, receiver) = std::sync::mpsc::channel::<Option<Walk>>();
                app.walk.rx = Some(receiver);
                app.curr_screen = Screen::Loading;
                std::thread::spawn(move || {
                    let walker = Walk::populate();
                    let _ = sender.send(walker);
                });
            }
            _ => (),
        }
    }
}

fn handle_sort_options(app: &mut App, key: KeyEvent) {
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

fn handle_clean_options(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Esc {
        app.curr_screen = Screen::Main;
        app.clean.focus = Row::Path;
        return;
    } else if key.code == KeyCode::Down || key.code == KeyCode::Tab {
        app.clean.next();
        return;
    } else if key.code == KeyCode::Up || key.code == KeyCode::BackTab {
        app.clean.prev();
        return;
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

fn handle_clean_results(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Esc {
        *app = App::new();
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

        if key.code == KeyCode::Char('d') && !app.result_state.fail_focus {
            app.result_state.is_popup = !app.result_state.is_popup;
        }
    } else {
        // the delete result is showing, wait for it to close by itself
        if app.result_state.delete_msg.is_some() {
            return;
        }
        if key.code == KeyCode::Char('y') {
            let curr_index = app.result_state.pos_succ.selected();
            if curr_index.is_none() {
                app.result_state.is_popup = !app.result_state.is_popup;
                return;
            }
            let curr_index = curr_index.unwrap();
            let report = app.result_state.output.as_mut();
            if report.is_none() {
                app.result_state.is_popup = !app.result_state.is_popup;
                return;
            } else if let Some(valid_report) = report {
                let path = valid_report.success[curr_index].1.clone();
                let msg = match delete_file(&path) {
                    Ok(()) => {
                        if curr_index == valid_report.success.len() - 1 {
                            if curr_index > 0 {
                                app.result_state.pos_succ.select(Some(curr_index - 1));
                            } else {
                                app.result_state.pos_succ.select(None);
                            }
                        }
                        valid_report.success.remove(curr_index);
                        Ok(format!("Deleted {}", path.display()))
                    }
                    Err(e) => Err(format!("Could not delete {}: {e}", path.display())),
                };
                // the popup stays open to show the message, expire_delete_msg closes it
                app.result_state.delete_msg = Some((msg, Instant::now()));
            }
        } else if key.code == KeyCode::Char('n') {
            app.result_state.is_popup = !app.result_state.is_popup;
        }
    }
}

fn handle_log(app: &mut App, key: KeyEvent) {
    let len = app.run_state.log_lines.len();
    if key.code == KeyCode::Char('q') {
        app.exit = true;
    } else if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
        app.log.next(len);
    } else if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
        app.log.prev(len);
    } else if key.code == KeyCode::Esc {
        app.curr_screen = Screen::CleanResults
    } else if key.code == KeyCode::PageUp {
        app.log.pgup(len);
    } else if key.code == KeyCode::PageDown {
        app.log.pgdown(len);
    }
}

fn handle_walk(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Char('q') {
        app.exit = true;
    }
    if !app.walk.is_popup {
        if key.code == KeyCode::Esc {
            *app = App::new();
        } else if key.code == KeyCode::Enter || key.code == KeyCode::Char('l') {
            app.walk.enter();
        } else if key.code == KeyCode::Char('h') {
            app.walk.leave();
        } else if key.code == KeyCode::Down
            || key.code == KeyCode::Tab
            || key.code == KeyCode::Char('j')
        {
            app.walk.next();
        } else if key.code == KeyCode::Up
            || key.code == KeyCode::BackTab
            || key.code == KeyCode::Char('k')
        {
            app.walk.prev();
        } else if key.code == KeyCode::PageUp {
            app.walk.begin();
        } else if key.code == KeyCode::PageDown {
            app.walk.end();
        } else if key.code == KeyCode::Char('d')
            && app
                .walk
                .list
                .selected()
                .and_then(|i| app.walk.current().children.as_ref()?.get(i))
                .is_some_and(|c| !c.is_dir)
        {
            app.walk.is_popup = !app.walk.is_popup;
        }
    } else {
        if key.code == KeyCode::Char('y') {
            // We don't want to double delete
            if app.walk.delete_msg.is_some() {
                return;
            }
            app.walk.handle_delete();
        } else if key.code == KeyCode::Char('n') {
            app.walk.is_popup = !app.walk.is_popup;
        }
    }
}
