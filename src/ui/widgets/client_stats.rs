use crate::state::AppState;
use chrono::Utc;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::Frame;
use unifi_rs::ClientOverview;
use uuid::Uuid;

pub struct ClientStatsView<'a> {
    client_id: Uuid,
    app_state: &'a AppState,
}

impl<'a> ClientStatsView<'a> {
    pub fn new(client_id: Uuid, app_state: &'a AppState) -> Self {
        Self {
            client_id,
            app_state,
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if let Some(client) = self.app_state.clients.iter().find(|c| match c {
            ClientOverview::Wireless(w) => w.base.id == self.client_id,
            ClientOverview::Wired(w) => w.base.id == self.client_id,
            _ => false,
        }) {
            match client {
                ClientOverview::Wireless(wireless) => {
                    self.render_wireless_stats(f, area, wireless);
                }
                ClientOverview::Wired(wired) => {
                    self.render_wired_stats(f, area, wired);
                }
                _ => {}
            }
        }
    }

    fn format_duration(connected_at: chrono::DateTime<Utc>) -> String {
        let duration = Utc::now().signed_duration_since(connected_at);
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        let seconds = duration.num_seconds() % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    fn render_wireless_stats(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WirelessClientOverview,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Connection info
                Constraint::Length(3), // Duration gauge
                Constraint::Length(7), // Device info
                Constraint::Min(0),    // Network stats
            ])
            .split(area);

        let info_text = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default()),
                Span::styled(
                    client.base.name.as_deref().unwrap_or("Unnamed"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("MAC Address: ", Style::default()),
                Span::styled(&client.mac_address, Style::default()),
            ]),
            Line::from(vec![
                Span::styled("IP Address: ", Style::default()),
                Span::styled(
                    client.base.ip_address.as_deref().unwrap_or("Unknown"),
                    Style::default(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Connected Since: ", Style::default()),
                Span::styled(
                    client
                        .base
                        .connected_at
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string(),
                    Style::default(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Session Duration: ", Style::default()),
                Span::styled(
                    Self::format_duration(client.base.connected_at),
                    Style::default(),
                ),
            ]),
        ];

        let info = Paragraph::new(info_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default())
                .title("Connection Info"),
        );
        f.render_widget(info, chunks[0]);

        let uptime = Utc::now().signed_duration_since(client.base.connected_at);
        let uptime_pct = (uptime.num_minutes() as f64 / (24.0 * 60.0)).min(1.0);
        let uptime_gauge = Gauge::default()
            .block(
                Block::default()
                    .title("Session Duration (% of 24h)")
                    .borders(Borders::ALL)
                    .border_style(Style::default()),
            )
            .gauge_style(Style::default())
            .ratio(uptime_pct)
            .label(format!("{:.1}%", uptime_pct * 100.0));
        f.render_widget(uptime_gauge, chunks[1]);

        if let Some(device) = self
            .app_state
            .devices
            .iter()
            .find(|d| d.id == client.uplink_device_id)
        {
            let device_text = vec![
                Line::from(vec![
                    Span::styled("Connected to: ", Style::default()),
                    Span::styled(&device.name, Style::default().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Model: ", Style::default()),
                    Span::styled(&device.model, Style::default()),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default()),
                    Span::styled(
                        format!("{:?}", device.state),
                        match device.state {
                            unifi_rs::DeviceState::Online => Style::default(),
                            unifi_rs::DeviceState::Offline => Style::default(),
                            _ => Style::default(),
                        },
                    ),
                ]),
            ];

            let device_info = Paragraph::new(device_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default())
                    .title("Connected Device"),
            );
            f.render_widget(device_info, chunks[2]);
        }
    }

    fn render_wired_stats(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WiredClientOverview,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Connection info
                Constraint::Length(3), // Duration gauge
                Constraint::Length(7), // Device info
                Constraint::Min(0),    // Network stats
            ])
            .split(area);

        let info_text = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default()),
                Span::styled(
                    client.base.name.as_deref().unwrap_or("Unnamed"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("MAC Address: ", Style::default()),
                Span::styled(&client.mac_address, Style::default()),
            ]),
            Line::from(vec![
                Span::styled("IP Address: ", Style::default()),
                Span::styled(
                    client.base.ip_address.as_deref().unwrap_or("Unknown"),
                    Style::default(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Connected Since: ", Style::default()),
                Span::styled(
                    client
                        .base
                        .connected_at
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string(),
                    Style::default(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Session Duration: ", Style::default()),
                Span::styled(
                    Self::format_duration(client.base.connected_at),
                    Style::default(),
                ),
            ]),
        ];

        let info = Paragraph::new(info_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default())
                .title("Connection Info"),
        );
        f.render_widget(info, chunks[0]);

        // Duration Gauge
        let uptime = Utc::now().signed_duration_since(client.base.connected_at);
        let uptime_pct = (uptime.num_minutes() as f64 / (24.0 * 60.0)).min(1.0);
        let uptime_gauge = Gauge::default()
            .block(
                Block::default()
                    .title("Session Duration (% of 24h)")
                    .borders(Borders::ALL)
                    .border_style(Style::default()),
            )
            .gauge_style(Style::default())
            .ratio(uptime_pct)
            .label(format!("{:.1}%", uptime_pct * 100.0));
        f.render_widget(uptime_gauge, chunks[1]);

        // Connected Device Info
        if let Some(device) = self
            .app_state
            .devices
            .iter()
            .find(|d| d.id == client.uplink_device_id)
        {
            let device_text = vec![
                Line::from(vec![
                    Span::styled("Connected to: ", Style::default()),
                    Span::styled(&device.name, Style::default().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Model: ", Style::default()),
                    Span::styled(&device.model, Style::default()),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default()),
                    Span::styled(
                        format!("{:?}", device.state),
                        match device.state {
                            unifi_rs::DeviceState::Online => Style::default(),
                            unifi_rs::DeviceState::Offline => Style::default(),
                            _ => Style::default(),
                        },
                    ),
                ]),
            ];

            let device_info = Paragraph::new(device_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default())
                    .title("Connected Device"),
            );
            f.render_widget(device_info, chunks[2]);
        }
    }
}
