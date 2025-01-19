use crate::state::AppState;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table, Tabs,
};
use ratatui::Frame;
use unifi_rs::{DeviceState, FrequencyBand, PortState, WlanStandard};
use uuid::Uuid;

pub struct DeviceStatsView {
    pub device_id: Uuid,
    pub current_tab: usize,
}

impl DeviceStatsView {
    pub fn new(device_id: Uuid, initial_tab: usize) -> Self {
        Self {
            device_id,
            current_tab: initial_tab,
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        let device = if let Some(device) = app_state.device_details.get(&self.device_id) {
            device
        } else {
            return;
        };

        let stats = app_state.device_stats.get(&self.device_id);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title bar
                Constraint::Length(3),  // Tabs
                Constraint::Min(0),     // Content
            ])
            .split(area);

        let status_style = match device.state {
            DeviceState::Online => Style::default().fg(Color::Green),
            DeviceState::Offline => Style::default().fg(Color::Red),
            _ => Style::default().fg(Color::Yellow),
        };

        let title = format!("{} - {}", device.name, device.model);
        let status_text = format!("{:?}", device.state);
        let uptime = stats.map_or("N/A".to_string(), |s| {
            let hours = s.uptime_sec / 3600;
            if hours > 24 {
                format!("{}d {}h", hours / 24, hours % 24)
            } else {
                format!("{}h", hours)
            }
        });

        let header_text = vec![Line::from(vec![
            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled(status_text, status_style),
            Span::raw(" | "),
            Span::raw(format!("Uptime: {}", uptime)),
        ])];

        let header = Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(header, chunks[0]);
        
        
        let is_access_point = device.features.as_ref()
            .map(|f| f.access_point.is_some())
            .unwrap_or(false);

        let titles = ["Overview",
            "Performance",
            if is_access_point { "Wireless" } else { "Network" },
            "Ports"];

        let tabs = Tabs::new(titles.iter().map(|t| Line::from(*t)).collect::<Vec<_>>())
            .block(Block::default().borders(Borders::ALL))
            .select(self.current_tab)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .divider("|");

        f.render_widget(tabs, chunks[1]);

        match self.current_tab {
            0 => self.render_overview(f, chunks[2], app_state),
            1 => self.render_performance(f, chunks[2], app_state),
            2 => {
                if is_access_point {
                    self.render_wireless(f, chunks[2], app_state)
                } else {
                    self.render_network(f, chunks[2], app_state)
                }
            },
            3 => self.render_ports(f, chunks[2], app_state),
            _ => {},
        }
    }

    fn render_overview(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),  // Basic info
                    Constraint::Length(8),  // Resources
                    Constraint::Min(0),     // Features
                ])
                .split(area);

            let info_text = vec![
                Line::from(vec![
                    Span::raw("MAC Address: "),
                    Span::styled(&device.mac_address, Style::default().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::raw("IP Address:  "),
                    Span::styled(&device.ip_address, Style::default().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::raw("Firmware:    "),
                    Span::styled(&device.firmware_version, Style::default().add_modifier(Modifier::BOLD)),
                    if device.firmware_updatable {
                        Span::styled(" (Update Available)", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("")
                    },
                ]),
                Line::from(vec![
                    Span::raw("Adopted:     "),
                    Span::styled(
                        device.adopted_at.map_or("Never".to_string(), |dt|
                            dt.format("%Y-%m-%d %H:%M:%S").to_string()
                        ),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
            ];

            let info_block = Paragraph::new(info_text)
                .block(Block::default().borders(Borders::ALL).title("Device Information"));
            f.render_widget(info_block, chunks[0]);

            if let Some(stats) = app_state.device_stats.get(&self.device_id) {
                let resources_text = vec![
                    Line::from(vec![
                        Span::raw("CPU Usage:    "),
                        Span::styled(
                            format!("{:.1}%", stats.cpu_utilization_pct.unwrap_or(0.0)),
                            self.get_usage_style(stats.cpu_utilization_pct.unwrap_or(0.0)),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("Memory Usage: "),
                        Span::styled(
                            format!("{:.1}%", stats.memory_utilization_pct.unwrap_or(0.0)),
                            self.get_usage_style(stats.memory_utilization_pct.unwrap_or(0.0)),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("Load Average: "),
                        Span::styled(
                            format!("{:.2} {:.2} {:.2}",
                                    stats.load_average_1min.unwrap_or(0.0),
                                    stats.load_average_5min.unwrap_or(0.0),
                                    stats.load_average_15min.unwrap_or(0.0)
                            ),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]),
                ];

                let resources_block = Paragraph::new(resources_text)
                    .block(Block::default().borders(Borders::ALL).title("Resource Utilization"));
                f.render_widget(resources_block, chunks[1]);
            }
            
            let mut feature_list = Vec::new();
            if let Some(features) = &device.features {
                if features.switching.is_some() {
                    feature_list.push("Switching");
                }
                if features.access_point.is_some() {
                    feature_list.push("Access Point");
                }
            }

            let features_text = vec![
                Line::from("Available Features:"),
                Line::from(""),
                Line::from(feature_list.join(", ")),
            ];

            let features_block = Paragraph::new(features_text)
                .block(Block::default().borders(Borders::ALL).title("Capabilities"));
            f.render_widget(features_block, chunks[2]);
        }
    }

    fn get_usage_style(&self, value: f64) -> Style {
        match value {
            v if v >= 90.0 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            v if v >= 75.0 => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            v if v >= 50.0 => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            _ => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        }
    }

    fn render_performance(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Current throughput
                Constraint::Min(0),    // Graph
            ])
            .split(area);

        if let Some(stats) = app_state.device_stats.get(&self.device_id) {
            if let Some(uplink) = &stats.uplink {
                let current_text = vec![Line::from(vec![
                    Span::raw("Current Throughput: "),
                    Span::styled(
                        format!("↑ {:.1} Mbps", uplink.tx_rate_bps as f64 / 1_000_000.0),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw(" / "),
                    Span::styled(
                        format!("↓ {:.1} Mbps", uplink.rx_rate_bps as f64 / 1_000_000.0),
                        Style::default().fg(Color::Blue),
                    ),
                ])];

                let current_stats = Paragraph::new(current_text)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(current_stats, chunks[0]);
            }
        }

        if let Some(history) = app_state.network_history.get(&self.device_id) {
            let history_vec: Vec<_> = history.iter().collect();

            if !history_vec.is_empty() {
                let tx_data: Vec<(f64, f64)> = history_vec
                    .iter()
                    .enumerate()
                    .map(|(i, point)| (i as f64, point.tx_rate))
                    .collect();

                let rx_data: Vec<(f64, f64)> = history_vec
                    .iter()
                    .enumerate()
                    .map(|(i, point)| (i as f64, point.rx_rate))
                    .collect();

                let max_rate = history_vec
                    .iter()
                    .map(|point| point.tx_rate.max(point.rx_rate))
                    .fold(0.0, f64::max);

                let datasets = vec![
                    Dataset::default()
                        .name("Upload")
                        .marker(symbols::Marker::Dot)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Green))
                        .data(&tx_data),
                    Dataset::default()
                        .name("Download")
                        .marker(symbols::Marker::Dot)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Blue))
                        .data(&rx_data),
                ];

                let chart = Chart::new(datasets)
                    .block(Block::default().title("Network History").borders(Borders::ALL))
                    .x_axis(
                        Axis::default()
                            .title("Time")
                            .bounds([0.0, 59.0])
                            .labels(vec![Line::from("5m ago"), Line::from("now")])
                    )
                    .y_axis(
                        Axis::default()
                            .title("Mbps")
                            .bounds([0.0, max_rate * 1.1])
                    );

                f.render_widget(chart, chunks[1]);
            }
        }
    }

    fn render_wireless(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            if let Some(interfaces) = &device.interfaces {
                let radios = &interfaces.radios;

                let header = Row::new(vec![
                    "Band",
                    "Channel",
                    "Width",
                    "Standard",
                    "Retries",
                ])
                    .style(Style::default().add_modifier(Modifier::BOLD));

                let rows: Vec<Row> = radios
                    .iter()
                    .map(|radio| {
                        let freq = radio.frequency_ghz.as_ref().map_or("Unknown", |f| match f {
                            FrequencyBand::Band2_4GHz => "2.4 GHz",
                            FrequencyBand::Band5GHz => "5 GHz",
                            FrequencyBand::Band6GHz => "6 GHz",
                            FrequencyBand::Band60GHz => "60 GHz",
                        });

                        let standard = radio.wlan_standard.as_ref().map_or("Unknown".to_string(), |s| match s {
                            WlanStandard::IEEE802_11A => "802.11a",
                            WlanStandard::IEEE802_11B => "802.11b",
                            WlanStandard::IEEE802_11G => "802.11g",
                            WlanStandard::IEEE802_11N => "802.11n",
                            WlanStandard::IEEE802_11AC => "802.11ac",
                            WlanStandard::IEEE802_11AX => "802.11ax",
                            WlanStandard::IEEE802_11BE => "802.11be",
                        }.to_string());

                        let retry_pct = if let Some(stats) = app_state.device_stats.get(&self.device_id) {
                            if let Some(interfaces) = &stats.interfaces {
                                if let Some(radio_stat) = interfaces.radios.iter()
                                    .find(|r| r.frequency_ghz == radio.frequency_ghz)
                                {
                                    radio_stat.tx_retries_pct
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let retry_cell = match retry_pct {
                            Some(pct) => {
                                let style = match pct {
                                    p if p > 15.0 => Style::default().fg(Color::Red),
                                    p if p > 5.0 => Style::default().fg(Color::Yellow),
                                    _ => Style::default().fg(Color::Green),
                                };
                                Cell::from(format!("{:.1}%", pct)).style(style)
                            },
                            None => Cell::from("N/A"),
                        };

                        Row::new(vec![
                            Cell::from(freq),
                            Cell::from(radio.channel.map_or("--".to_string(), |c| c.to_string())),
                            Cell::from(radio.channel_width_mhz.map_or("--".to_string(), |w| format!("{} MHz", w))),
                            Cell::from(standard),
                            retry_cell,
                        ])
                    })
                    .collect();

                let widths = [
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                ];

                let table = Table::new(rows, widths)
                    .header(header)
                    .block(Block::default().title("Radio Information").borders(Borders::ALL));

                f.render_widget(table, area);
            }
        }
    }

    fn render_network(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Current throughput
                Constraint::Min(0),    // Graph
            ])
            .split(area);
        
        if let Some(stats) = app_state.device_stats.get(&self.device_id) {
            if let Some(uplink) = &stats.uplink {
                let current_text = vec![Line::from(vec![
                    Span::raw("Current Throughput: "),
                    Span::styled(
                        format!("↑ {:.1} Mbps", uplink.tx_rate_bps  as f64 / 1_000_000.0),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw(" / "),
                    Span::styled(
                        format!("↓ {:.1} Mbps", uplink.rx_rate_bps  as f64 / 1_000_000.0),
                        Style::default().fg(Color::Blue),
                    ),
                ])];

                let current_stats = Paragraph::new(current_text)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(current_stats, chunks[0]);
            }
        }

        
        if let Some(history) = app_state.network_history.get(&self.device_id) {
            let history_vec: Vec<_> = history.iter().collect();

            if !history_vec.is_empty() {
                let tx_data: Vec<(f64, f64)> = history_vec
                    .iter()
                    .enumerate()
                    .map(|(i, point)| (i as f64, point.tx_rate))
                    .collect();

                let rx_data: Vec<(f64, f64)> = history_vec
                    .iter()
                    .enumerate()
                    .map(|(i, point)| (i as f64, point.rx_rate))
                    .collect();

                let max_rate = history_vec
                    .iter()
                    .map(|point| point.tx_rate.max(point.rx_rate))
                    .fold(0.0, f64::max);

                let datasets = vec![
                    Dataset::default()
                        .name("Upload")
                        .marker(symbols::Marker::Dot)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Green))
                        .data(&tx_data),
                    Dataset::default()
                        .name("Download")
                        .marker(symbols::Marker::Dot)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Blue))
                        .data(&rx_data),
                ];

                let chart = Chart::new(datasets)
                    .block(Block::default().title("Network History").borders(Borders::ALL))
                    .x_axis(
                        Axis::default()
                            .title("Time")
                            .bounds([0.0, 59.0])
                            .labels(vec![Line::from("5m ago"), Line::from("now")])
                    )
                    .y_axis(
                        Axis::default()
                            .title("Mbps")
                            .bounds([0.0, max_rate * 1.1])
                    );

                f.render_widget(chart, chunks[1]);
            }
        }
    }

    fn render_ports(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            if let Some(interfaces) = &device.interfaces {
                if !interfaces.ports.is_empty() {
                    let header = Row::new(vec![
                        "Port",
                        "Type",
                        "Status",
                        "Speed",
                        "Max Speed",
                    ])
                        .style(Style::default().add_modifier(Modifier::BOLD));

                    let rows: Vec<Row> = interfaces.ports.iter()
                        .map(|port| {
                            let status_style = match port.state {
                                PortState::Up => Style::default().fg(Color::Green),
                                PortState::Down => Style::default().fg(Color::Red),
                                PortState::Unknown => Style::default().fg(Color::Yellow),
                            };

                            let speed_text = if port.speed_mbps >= 1000 {
                                format!("{} Gbps", port.speed_mbps / 1000)
                            } else {
                                format!("{} Mbps", port.speed_mbps)
                            };

                            let max_speed_text = if port.max_speed_mbps >= 1000 {
                                format!("{} Gbps", port.max_speed_mbps / 1000)
                            } else {
                                format!("{} Mbps", port.max_speed_mbps)
                            };

                            Row::new(vec![
                                Cell::from(port.idx.to_string()),
                                Cell::from(format!("{:?}", port.connector)),
                                Cell::from(format!("{:?}", port.state)).style(status_style),
                                Cell::from(speed_text),
                                Cell::from(max_speed_text),
                            ])
                        })
                        .collect();

                    let widths = [
                        Constraint::Percentage(15),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                        Constraint::Percentage(25),
                    ];

                    let table = Table::new(rows, widths)
                        .header(header)
                        .block(Block::default().title("Port Status").borders(Borders::ALL))
                        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

                    f.render_widget(table, area);
                }
            }
        }
    }
}