use crate::state::AppState;
use chrono::{DateTime, Utc};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
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
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(9),  // Connection info
                        Constraint::Length(12), // Device/Radio info or Port status
                        Constraint::Min(0),     // Network stats and charts
                    ]
                    .as_ref(),
                )
                .split(area);

            match client {
                ClientOverview::Wireless(wireless) => {
                    self.render_connection_info(f, chunks[0], wireless);
                    self.render_wireless_device_info(f, chunks[1], wireless);
                }
                ClientOverview::Wired(wired) => {
                    self.render_wired_connection_info(f, chunks[0], wired);
                    self.render_wired_device_info(f, chunks[1], wired);
                }
                _ => {}
            }
        }
    }

    fn format_duration(connected_at: DateTime<Utc>) -> (String, Style) {
        let duration = Utc::now().signed_duration_since(connected_at);
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        let seconds = duration.num_seconds() % 60;

        let style = if hours >= 24 {
            Style::default().fg(Color::Green)
        } else if hours >= 1 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Blue)
        };

        let formatted = if hours >= 24 {
            let days = hours / 24;
            let remaining_hours = hours % 24;
            format!("{} days, {} hours", days, remaining_hours)
        } else if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m {}s", minutes, seconds)
        };

        (formatted, style)
    }

    fn render_connection_info(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WirelessClientOverview,
    ) {
        let (duration, duration_style) = Self::format_duration(client.base.connected_at);

        let info_text = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default()),
                Span::styled(
                    client.base.name.as_deref().unwrap_or("Unnamed"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" ("),
                Span::styled("Wireless", Style::default().fg(Color::Yellow)),
                Span::raw(")"),
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
            Line::from(""),
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
                Span::styled(duration, duration_style),
            ]),
        ];

        let connection_block = Block::default()
            .borders(Borders::ALL)
            .title("Connection Information");

        let info = Paragraph::new(info_text)
            .block(connection_block)
            .style(Style::default());

        f.render_widget(info, area);
    }

    fn render_wired_connection_info(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WiredClientOverview,
    ) {
        let (duration, duration_style) = Self::format_duration(client.base.connected_at);

        let info_text = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default()),
                Span::styled(
                    client.base.name.as_deref().unwrap_or("Unnamed"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" ("),
                Span::styled("Wired", Style::default().fg(Color::Blue)),
                Span::raw(")"),
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
            Line::from(""),
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
                Span::styled(duration, duration_style),
            ]),
        ];

        let connection_block = Block::default()
            .borders(Borders::ALL)
            .title("Connection Information");

        let info = Paragraph::new(info_text)
            .block(connection_block)
            .style(Style::default());

        f.render_widget(info, area);
    }

    fn render_wireless_device_info(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WirelessClientOverview,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Device info
                Constraint::Min(0),    // Radio table
            ])
            .split(area);

        if let Some(device) = self
            .app_state
            .devices
            .iter()
            .find(|d| d.id == client.uplink_device_id)
        {
            let device_text = if let Some(details) = self.app_state.device_details.get(&device.id) {
                vec![
                    Line::from(vec![
                        Span::styled("Access Point: ", Style::default()),
                        Span::styled(&device.name, Style::default().add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("Model: ", Style::default()),
                        Span::styled(&device.model, Style::default()),
                        Span::raw(" | "),
                        Span::styled("Firmware: ", Style::default()),
                        Span::styled(&details.firmware_version, Style::default()),
                    ]),
                    Line::from(vec![
                        Span::styled("Status: ", Style::default()),
                        Span::styled(
                            format!("{:?}", device.state),
                            match device.state {
                                unifi_rs::DeviceState::Online => Style::default().fg(Color::Green),
                                unifi_rs::DeviceState::Offline => Style::default().fg(Color::Red),
                                _ => Style::default().fg(Color::Yellow),
                            },
                        ),
                    ]),
                ]
            } else {
                vec![Line::from(format!("Access Point: {}", device.name))]
            };

            let device_info = Paragraph::new(device_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Access Point Information"),
            );
            f.render_widget(device_info, chunks[0]);

            if let Some(details) = self.app_state.device_details.get(&device.id) {
                if let Some(interfaces) = &details.interfaces {
                    let header = Row::new(vec!["Band", "Channel", "Width", "Quality"])
                        .style(Style::default().add_modifier(Modifier::BOLD));

                    let rows: Vec<Row> = interfaces
                        .radios
                        .iter()
                        .map(|radio| {
                            let freq =
                                radio.frequency_ghz.as_ref().map_or("Unknown", |f| match f {
                                    unifi_rs::FrequencyBand::Band2_4GHz => "2.4 GHz",
                                    unifi_rs::FrequencyBand::Band5GHz => "5 GHz",
                                    unifi_rs::FrequencyBand::Band6GHz => "6 GHz",
                                    unifi_rs::FrequencyBand::Band60GHz => "60 GHz",
                                });

                            let channel = radio.channel.map_or("--".to_string(), |c| c.to_string());
                            let width = radio
                                .channel_width_mhz
                                .map_or("--".to_string(), |w| format!("{} MHz", w));

                            let quality = if let Some(stats) =
                                self.app_state.device_stats.get(&device.id)
                            {
                                if let Some(interfaces) = &stats.interfaces {
                                    if let Some(radio_stat) = interfaces
                                        .radios
                                        .iter()
                                        .find(|r| r.frequency_ghz == radio.frequency_ghz)
                                    {
                                        let retry_pct = radio_stat.tx_retries_pct.unwrap_or(0.0);
                                        if retry_pct > 15.0 {
                                            Cell::from("Poor")
                                                .style(Style::default().fg(Color::Red))
                                        } else if retry_pct > 5.0 {
                                            Cell::from("Fair")
                                                .style(Style::default().fg(Color::Yellow))
                                        } else {
                                            Cell::from("Good")
                                                .style(Style::default().fg(Color::Green))
                                        }
                                    } else {
                                        Cell::from("--")
                                    }
                                } else {
                                    Cell::from("--")
                                }
                            } else {
                                Cell::from("--")
                            };

                            Row::new(vec![
                                Cell::from(freq),
                                Cell::from(channel),
                                Cell::from(width),
                                quality,
                            ])
                        })
                        .collect();

                    let width = vec![
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ];

                    let table = Table::new(rows, width).header(header).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Radio Information"),
                    );

                    f.render_widget(table, chunks[1]);
                }
            }
        }
    }

    fn render_wired_device_info(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WiredClientOverview,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Device info
                Constraint::Min(0),    // Port table
            ])
            .split(area);

        if let Some(device) = self
            .app_state
            .devices
            .iter()
            .find(|d| d.id == client.uplink_device_id)
        {
            let device_text = if let Some(details) = self.app_state.device_details.get(&device.id) {
                vec![
                    Line::from(vec![
                        Span::styled("Switch: ", Style::default()),
                        Span::styled(&device.name, Style::default().add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("Model: ", Style::default()),
                        Span::styled(&device.model, Style::default()),
                        Span::raw(" | "),
                        Span::styled("Firmware: ", Style::default()),
                        Span::styled(&details.firmware_version, Style::default()),
                    ]),
                    Line::from(vec![
                        Span::styled("Status: ", Style::default()),
                        Span::styled(
                            format!("{:?}", device.state),
                            match device.state {
                                unifi_rs::DeviceState::Online => Style::default().fg(Color::Green),
                                unifi_rs::DeviceState::Offline => Style::default().fg(Color::Red),
                                _ => Style::default().fg(Color::Yellow),
                            },
                        ),
                    ]),
                ]
            } else {
                vec![Line::from(format!("Switch: {}", device.name))]
            };

            let device_info = Paragraph::new(device_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Switch Information"),
            );
            f.render_widget(device_info, chunks[0]);

            // Port Information Table
            if let Some(details) = self.app_state.device_details.get(&device.id) {
                if let Some(interfaces) = &details.interfaces {
                    let header = Row::new(vec!["Port", "Type", "Speed", "Status"])
                        .style(Style::default().add_modifier(Modifier::BOLD));

                    let rows: Vec<Row> = interfaces
                        .ports
                        .iter()
                        .map(|port| {
                            let port_type = format!("{:?}", port.connector);

                            let speed = if port.speed_mbps > 0 {
                                if port.speed_mbps >= 1000 {
                                    format!("{} Gbps", port.speed_mbps / 1000)
                                } else {
                                    format!("{} Mbps", port.speed_mbps)
                                }
                            } else {
                                "No Link".to_string()
                            };

                            let status_style = match port.state {
                                unifi_rs::PortState::Up => Style::default().fg(Color::Green),
                                unifi_rs::PortState::Down => Style::default().fg(Color::Red),
                                _ => Style::default().fg(Color::Yellow),
                            };

                            Row::new(vec![
                                Cell::from(port.idx.to_string()),
                                Cell::from(port_type),
                                Cell::from(speed),
                                Cell::from(format!("{:?}", port.state)).style(status_style),
                            ])
                        })
                        .collect();

                    let width = vec![
                        Constraint::Percentage(15),
                        Constraint::Percentage(35),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ];
                    let table = Table::new(rows, width).header(header).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Port Information"),
                    );
                    f.render_widget(table, chunks[1]);
                }
            }
        }
    }
}
