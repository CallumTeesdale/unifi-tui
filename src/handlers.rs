use crate::app::{App, DialogType};
use crate::error::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub async fn handle_global_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            Ok(true)
        }
        KeyCode::Char('?') => {
            app.toggle_help();
            Ok(true)
        }
        KeyCode::Char('/') => {
            app.enter_search_mode();
            Ok(true)
        }
        KeyCode::Esc if !app.search_mode && !app.search_query.is_empty() => {
            app.clear_search();
            Ok(true)
        }
        KeyCode::Tab => {
            app.next_tab();
            Ok(true)
        }
        KeyCode::BackTab => {
            app.previous_tab();
            Ok(true)
        }
        KeyCode::Char('r') => {
            app.state.last_update -= app.state.refresh_interval;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub async fn handle_dialog_input(app: &mut App, key: KeyEvent) -> Result<()> {
    let dialog = app.dialog.take().unwrap();
    match key.code {
        KeyCode::Char('y') if dialog.dialog_type == DialogType::Confirmation => {
            if let Some(callback) = dialog.callback {
                if let Err(e) = callback(app) {
                    app.state.set_error(format!("Operation failed: {}", e));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn handle_search_input(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.exit_search_mode();
        }
        KeyCode::Enter => {
            app.exit_search_mode();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.state.search(&app.search_query);
        }
        KeyCode::Backspace => {
            if !app.search_query.is_empty() {
                app.search_query.pop();
                app.state.search(&app.search_query);
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn handle_device_detail_input(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.back_to_overview();
        }
        KeyCode::Tab => {
            if let Some(view) = app.device_stats_view.as_mut() {
                view.current_tab = (view.current_tab + 1) % 4;
            }
        }
        KeyCode::BackTab => {
            if let Some(view) = app.device_stats_view.as_mut() {
                view.current_tab = (view.current_tab + 3) % 4;
            }
        }
        KeyCode::Right => {
            if let Some(view) = app.device_stats_view.as_mut() {
                view.current_tab = (view.current_tab + 1) % 4;
            }
        }
        KeyCode::Left => {
            if let Some(view) = app.device_stats_view.as_mut() {
                view.current_tab = (view.current_tab + 3) % 4;
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn handle_client_detail_input(app: &mut App, key: KeyEvent) -> Result<()> {
    if key.code == KeyCode::Esc {
        app.back_to_overview();
    }
    Ok(())
}
