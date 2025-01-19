pub mod clients;
pub mod devices;
pub mod sites;
pub mod stats;
pub mod status_bar;
pub mod widgets;
use crate::app::{App, DialogType, Mode};
use crate::ui::{
    clients::render_clients, devices::render_devices, sites::render_sites, stats::render_stats,
    status_bar::render_status_bar,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs};
use ratatui::Frame;

pub fn render(app: &mut App, f: &mut Frame) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Status bar
            ]
                .as_ref(),
        )
        .split(size);

    render_tabs(f, app, chunks[0]);
    

    if app.dialog.is_some() {
        render_dialog(f, app, size);
    } else if app.show_help {
        render_help(f, app, chunks[1]);
    } else if app.search_mode {
        match app.mode {
            Mode::Overview => render_overview(f, app, chunks[1]),
            Mode::DeviceDetail => render_device_detail(f, app, chunks[1]),
            Mode::ClientDetail => render_client_detail(f, app, chunks[1]),
            Mode::Help => render_help(f, app, chunks[1]),
        }
        render_search(f, app, size);
    } else {
        match app.mode {
            Mode::Overview => render_overview(f, app, chunks[1]),
            Mode::DeviceDetail => render_device_detail(f, app, chunks[1]),
            Mode::ClientDetail => render_client_detail(f, app, chunks[1]),
            Mode::Help => render_help(f, app, chunks[1]),
        }
    }

    render_status_bar(f, app, chunks[2]);

    if let Some(error) = &app.state.error_message {
        if let Some(timestamp) = app.state.error_timestamp {
            if timestamp.elapsed() < std::time::Duration::from_secs(5) {
                render_error(f, error, size);
            }
        }
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = ["Sites", "Devices", "Clients", "Stats"];
    let tabs = Tabs::new(titles.iter().map(|t| Line::from(*t)).collect::<Vec<_>>())
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.current_tab)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Gray),
        );
    f.render_widget(tabs, area);
}

fn render_overview(f: &mut Frame, app: &mut App, area: Rect) {
    match app.current_tab {
        0 => render_sites(f, app, area),
        1 => render_devices(f, app, area),
        2 => render_clients(f, app, area),
        3 => render_stats(f, app, area),
        _ => unreachable!(),
    }
}

fn render_device_detail(f: &mut Frame, app: &App, area: Rect) {
    if let Some(_device_id) = app.selected_device_id {
        if let Some(view) = &app.device_stats_view {
            view.render(f, area, &app.state);
        }
    }
}
fn render_client_detail(f: &mut Frame, app: &App, area: Rect) {
    if let Some(client_id) = app.selected_client_id {
        widgets::client_stats::ClientStatsView::new(client_id, &app.state).render(f, area);
    }
}

pub fn render_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    if let Some(dialog) = &app.dialog {
        app.state.set_error(format!("Rendering dialog: {}", dialog.title));

        let dialog_area = centered_rect(60, 15, area);
        f.render_widget(Clear, dialog_area);

        let text = vec![
            Line::from(""),
            Line::from(dialog.message.clone()),
            Line::from(""),
            Line::from(match dialog.dialog_type {
                DialogType::Confirmation => "(y) Confirm  (n) Cancel",
                _ => "Press any key to close",
            }),
        ];

        let dialog_widget = Paragraph::new(text)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(dialog.title.clone()))
            .alignment(Alignment::Center);

        f.render_widget(dialog_widget, dialog_area);
    }
}



fn render_search(f: &mut Frame, app: &App, area: Rect) {
    let search_area = centered_rect(60, 3, area);

    let shadow_block = Block::default().style(Style::default());
    f.render_widget(Clear, search_area);
    f.render_widget(shadow_block, search_area);

    let search_text = Paragraph::new(app.search_query.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default())
                .title("Search (Esc to close)"),
        )
        .style(Style::default());

    f.render_widget(search_text, search_area);
}

fn render_error(f: &mut Frame, error: &str, area: Rect) {
    let area = centered_rect(60, 15, area);
    let error_widget = Paragraph::new(error)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default())
                .title("Error"),
        )
        .style(Style::default());
    f.render_widget(Clear, area);
    f.render_widget(error_widget, area);
}

fn render_help(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.mode {
        Mode::Overview => {
            match app.current_tab {
                0 => vec![
                    // Sites tab
                    Line::from("UniFi Network TUI Help - Sites View"),
                    Line::from(""),
                    Line::from("Global Commands:"),
                    Line::from("  q      - Quit application"),
                    Line::from("  ?      - Toggle this help screen"),
                    Line::from("  /      - Enter search mode"),
                    Line::from("  Tab    - Next view"),
                    Line::from("  S-Tab  - Previous view"),
                    Line::from("  F5     - Force refresh data"),
                    Line::from(""),
                    Line::from("Site Navigation:"),
                    Line::from("  ↑/↓    - Select site"),
                    Line::from("  Enter  - View selected site"),
                    Line::from("  Esc    - Show all sites"),
                ],
                1 => vec![
                    // Devices tab
                    Line::from("UniFi Network TUI Help - Devices View"),
                    Line::from(""),
                    Line::from("Global Commands:"),
                    Line::from("  q      - Quit application"),
                    Line::from("  ?      - Toggle this help screen"),
                    Line::from("  /      - Search devices by name, model, MAC, or IP"),
                    Line::from("  Tab    - Next view"),
                    Line::from("  S-Tab  - Previous view"),
                    Line::from("  F5     - Force refresh data"),
                    Line::from(""),
                    Line::from("Device Navigation:"),
                    Line::from("  ↑/↓    - Select device"),
                    Line::from("  Enter  - View device details"),
                    Line::from("  s      - Sort devices (cycles through sorting options)"),
                ],
                2 => vec![
                    // Clients tab
                    Line::from("UniFi Network TUI Help - Clients View"),
                    Line::from(""),
                    Line::from("Global Commands:"),
                    Line::from("  q      - Quit application"),
                    Line::from("  ?      - Toggle this help screen"),
                    Line::from("  /      - Search clients by name, MAC, or IP"),
                    Line::from("  Tab    - Next view"),
                    Line::from("  S-Tab  - Previous view"),
                    Line::from("  F5     - Force refresh data"),
                    Line::from(""),
                    Line::from("Client Navigation:"),
                    Line::from("  ↑/↓    - Select client"),
                    Line::from("  Enter  - View client details"),
                    Line::from("  s      - Sort clients (cycles through sorting options)"),
                ],
                3 => vec![
                    // Stats tab
                    Line::from("UniFi Network TUI Help - Statistics View"),
                    Line::from(""),
                    Line::from("Global Commands:"),
                    Line::from("  q      - Quit application"),
                    Line::from("  ?      - Toggle this help screen"),
                    Line::from("  Tab    - Next view"),
                    Line::from("  S-Tab  - Previous view"),
                    Line::from("  F5     - Force refresh data"),
                    Line::from(""),
                    Line::from("Statistics Information:"),
                    Line::from("  - Shows network overview and device metrics"),
                    Line::from("  - Updates every refresh cycle (5s by default)"),
                    Line::from("  - Maintains history of last 100 data points"),
                ],
                _ => vec![],
            }
        },
        _ => vec![Line::from("Help not available for this view")],
    };

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"));

    f.render_widget(help, area);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
