use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unifi_rs::DeviceState;

pub fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let online_devices = app
        .state
        .devices
        .iter()
        .filter(|d| matches!(d.state, DeviceState::Online))
        .count();

    let status = format!(
        "{} | Devices: {} ({} online) | Clients: {} | {}",
        app.state
            .selected_site
            .as_ref()
            .map_or("All Sites", |s| &s.site_name),
        app.state.devices.len(),
        online_devices,
        app.state.clients.len(),
        format_uptime(app.state.last_update.elapsed()),
    );

    let status_bar = Paragraph::new(status).style(Style::default());

    f.render_widget(status_bar, area);
}

fn format_uptime(duration: std::time::Duration) -> String {
    let uptime = duration.as_secs();
    let hours = uptime / 3600;
    let minutes = (uptime % 3600) / 60;
    let seconds = uptime % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
