use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let site_context = app
        .state
        .selected_site
        .as_ref()
        .map(|s| format!("Site: {} | ", s.site_name))
        .unwrap_or_else(|| "All Sites | ".to_string());

    let status = format!(
        "{} Sites: {} | Devices: {} | Clients: {} | Last Update: {}s ago {}",
        site_context,
        app.state.sites.len(),
        app.state.devices.len(),
        app.state.clients.len(),
        app.state.last_update.elapsed().as_secs(),
        if app.search_mode { "| SEARCH MODE" } else { "" }
    );

    let status_widget = Paragraph::new(Line::from(status))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    f.render_widget(status_widget, area);
}
