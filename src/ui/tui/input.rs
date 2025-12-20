use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::UiAction;
use crate::ui::{AlertFormField, AppState, InputMode};

/// Handle keyboard events for TUI, returning actions for the session manager
pub fn handle_key_event(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    if key_event.kind == KeyEventKind::Release {
        return UiAction::None;
    }

    // Global shortcut to bring up alert popup when in normal mode
    if matches!(app.input_mode, InputMode::Normal) {
        if let KeyCode::Char('a') = key_event.code {
            if !key_event.modifiers.contains(KeyModifiers::CONTROL) {
                let preset = app
                    .current_symbol()
                    .and_then(|sym| app.market_data.get(sym))
                    .map(|md| md.price);
                if let Err(e) = app.activate_alert_popup(preset) {
                    app.push_notification(e);
                }
                return UiAction::None;
            }
        }
    }

    // Global shortcuts first
    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        match key_event.code {
            KeyCode::Char('c') | KeyCode::Char('d') => {
                app.should_quit = true;
                return UiAction::QuitRequested;
            }
            KeyCode::Char('p') => {
                app.toggle_pause();
                return UiAction::None;
            }
            _ => {}
        }
    }

    match app.input_mode {
        InputMode::Normal => handle_normal_mode_keys(app, key_event),
        InputMode::Command => handle_command_mode_keys(app, key_event),
        InputMode::AlertPopup => handle_alert_popup_keys(app, key_event),
        InputMode::Alerts => handle_alerts_mode_keys(app, key_event),
    }
}

fn handle_normal_mode_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            UiAction::QuitRequested
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.clear_command();
            UiAction::None
        }
        KeyCode::Char('/') | KeyCode::Char(':') => {
            let preset = if matches!(key_event.code, KeyCode::Char('/')) {
                Some("/")
            } else {
                None
            };
            app.activate_command_mode(preset);
            UiAction::None
        }
        KeyCode::Char('p') | KeyCode::Char(' ') => {
            app.toggle_pause();
            UiAction::None
        }
        KeyCode::Left | KeyCode::Up => {
            app.previous_tab();
            UiAction::None
        }
        KeyCode::Right | KeyCode::Down => {
            app.next_tab();
            UiAction::None
        }
        KeyCode::Char('k') => {
            app.scroll_logs_up();
            UiAction::None
        }
        KeyCode::Char('j') => {
            app.scroll_logs_down();
            UiAction::None
        }
        KeyCode::Char('s') => {
            app.activate_command_mode(Some("/status"));
            UiAction::None
        }
        KeyCode::Char('L') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            app.activate_command_mode(Some("/logs"));
            UiAction::None
        }
        KeyCode::Char('A') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            app.enter_alerts_view();
            UiAction::SubmitCommand("/alert:list".to_string())
        }
        KeyCode::Enter => UiAction::None,
        _ => UiAction::None,
    }
}

fn handle_command_mode_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.clear_command();
            app.reset_command_suggestions();
            UiAction::None
        }
        KeyCode::Enter => {
            let command = app.command_buffer.trim().to_string();
            app.input_mode = InputMode::Normal;
            app.clear_command();
            app.reset_command_suggestions();
            if command.is_empty() {
                UiAction::None
            } else {
                UiAction::SubmitCommand(command)
            }
        }
        KeyCode::Backspace => {
            app.command_buffer.pop();
            app.update_command_suggestions();
            UiAction::None
        }
        KeyCode::Up => {
            app.select_previous_suggestion();
            UiAction::None
        }
        KeyCode::Down => {
            app.select_next_suggestion();
            UiAction::None
        }
        KeyCode::Tab => {
            app.apply_selected_suggestion();
            UiAction::None
        }
        KeyCode::Char(c) => {
            if !key_event.modifiers.contains(KeyModifiers::CONTROL) {
                app.command_buffer.push(c);
                app.update_command_suggestions();
            }
            UiAction::None
        }
        KeyCode::Left | KeyCode::Right => UiAction::None,
        _ => UiAction::None,
    }
}

fn handle_alerts_mode_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Esc => {
            app.exit_alerts_view();
            UiAction::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_alert();
            UiAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_alert();
            UiAction::None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            if let Some(alert) = app.selected_alert() {
                UiAction::SubmitCommand(format!("/alert:clear {}", alert.id))
            } else {
                UiAction::None
            }
        }
        KeyCode::Char('C') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            UiAction::SubmitCommand("/alert:clear all".to_string())
        }
        KeyCode::Char('r') => UiAction::SubmitCommand("/alert:list".to_string()),
        KeyCode::Char('q') => {
            app.exit_alerts_view();
            UiAction::None
        }
        _ => UiAction::None,
    }
}

fn handle_alert_popup_keys(app: &mut AppState, key_event: KeyEvent) -> UiAction {
    match key_event.code {
        KeyCode::Esc => {
            app.deactivate_alert_popup();
            UiAction::None
        }
        KeyCode::Up => {
            app.cycle_alert_popup_field(true);
            UiAction::None
        }
        KeyCode::Down => {
            app.cycle_alert_popup_field(false);
            UiAction::None
        }
        KeyCode::Tab => match app.alert_form.active_field {
            AlertFormField::Direction => {
                app.toggle_alert_direction();
                UiAction::None
            }
            AlertFormField::Mode => {
                app.toggle_alert_repeat();
                UiAction::None
            }
            _ => UiAction::None,
        },
        KeyCode::Enter => {
            let price = match app.alert_price() {
                Ok(price) => price,
                Err(e) => {
                    app.alert_form.error = Some(e);
                    return UiAction::None;
                }
            };
            let options = match app.alert_options(price) {
                Ok(options) => options,
                Err(e) => {
                    app.alert_form.error = Some(e);
                    return UiAction::None;
                }
            };
            let symbol = app.alert_form.symbol.clone();
            let direction = app.alert_direction();
            app.deactivate_alert_popup();
            UiAction::SubmitAlert {
                symbol,
                direction,
                price,
                options,
            }
        }
        KeyCode::Backspace => {
            match app.alert_form.active_field {
                AlertFormField::Price => {
                    if !app.alert_form.price_dirty {
                        app.alert_form.price_input.clear();
                        app.alert_form.price_dirty = true;
                    }
                    app.alert_form.price_input.pop();
                    app.alert_form.error = None;
                }
                AlertFormField::Cooldown => {
                    app.alert_form.cooldown_input.pop();
                    app.alert_form.error = None;
                }
                AlertFormField::Hysteresis => {
                    app.alert_form.hysteresis_input.pop();
                    app.alert_form.error = None;
                }
                _ => {}
            }
            UiAction::None
        }
        KeyCode::Char(c) => {
            match app.alert_form.active_field {
                AlertFormField::Price => {
                    if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
                        if !app.alert_form.price_dirty {
                            app.alert_form.price_input.clear();
                            app.alert_form.price_dirty = true;
                        }
                        app.alert_form.price_input.push(c);
                        app.alert_form.error = None;
                    }
                }
                AlertFormField::Cooldown => {
                    if c.is_ascii_digit() {
                        app.alert_form.cooldown_input.push(c);
                        app.alert_form.error = None;
                    }
                }
                AlertFormField::Hysteresis => {
                    if c.is_ascii_digit() || c == '.' || c == '%' {
                        app.alert_form.hysteresis_input.push(c);
                        app.alert_form.error = None;
                    }
                }
                _ => {}
            }
            UiAction::None
        }
        _ => UiAction::None,
    }
}
