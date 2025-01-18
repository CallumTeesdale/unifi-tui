use crate::state::{AppState, NetworkThroughput};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, Borders, Cell, Chart, Dataset, Gauge, GraphType, Paragraph, Row, Table, Tabs,
};
use ratatui::{symbols, Frame};
use unifi_rs::{FrequencyBand, PortState};
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3), // Tabs
                    Constraint::Min(0),    // Content
                ]
                .as_ref(),
            )
            .split(area);

        let titles = ["Overview", "Network", "Radio Stats", "Port Status"];
        let tabs = Tabs::new(titles.iter().map(|t| Line::from(*t)).collect::<Vec<_>>())
            .block(Block::default().borders(Borders::ALL))
            .select(self.current_tab)
            .highlight_style(Style::default().fg(Color::Cyan));

        f.render_widget(tabs, chunks[0]);

        match self.current_tab {
            0 => self.render_overview(f, chunks[1], app_state),
            1 => self.render_network_stats(f, chunks[1], app_state),
            2 => self.render_radio_stats(f, chunks[1], app_state),
            3 => self.render_port_status(f, chunks[1], app_state),
            _ => {}
        }
    }

    fn render_overview(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            if let Some(stats) = app_state.device_stats.get(&self.device_id) {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Constraint::Length(3), // CPU
                            Constraint::Length(3), // Memory
                            Constraint::Min(0),    // Features
                        ]
                        .as_ref(),
                    )
                    .split(area);

                let cpu_usage = format!("CPU: {:.1}%", stats.cpu_utilization_pct.unwrap_or(0.0));
                let memory_usage = format!(
                    "Memory: {:.1}%",
                    stats.memory_utilization_pct.unwrap_or(0.0)
                );

                let cpu = Gauge::default()
                    .block(Block::default().borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Cyan))
                    .ratio(stats.cpu_utilization_pct.unwrap_or(0.0) / 100.0)
                    .label(cpu_usage);

                let memory = Gauge::default()
                    .block(Block::default().borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Magenta))
                    .ratio(stats.memory_utilization_pct.unwrap_or(0.0) / 100.0)
                    .label(memory_usage);


                f.render_widget(cpu, chunks[0]);
                f.render_widget(memory, chunks[1]);
                
                let info_text = vec![
                    Line::from(vec![
                        Span::raw("Firmware: "),
                        Span::styled(
                            &device.firmware_version,
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("Uptime: "),
                        Span::styled(
                            format!("{} hours", stats.uptime_sec / 3600),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]),
                ];

                let info = Paragraph::new(info_text)
                    .block(Block::default().borders(Borders::ALL).title("Device Info"));
                f.render_widget(info, chunks[2]);
            }
        }
    }

    fn render_network_stats(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(history) = app_state.network_history.get(&self.device_id) {
            let history_vec: Vec<&NetworkThroughput> = history.iter().collect();

            if !history_vec.is_empty() {
                let tx_data: Vec<(f64, f64)> = history_vec.iter()
                    .enumerate()
                    .map(|(i, point)| (i as f64, point.tx_rate))
                    .collect();

                let rx_data: Vec<(f64, f64)> = history_vec.iter()
                    .enumerate()
                    .map(|(i, point)| (i as f64, point.rx_rate))
                    .collect();
                
                let max_rate = history_vec.iter()
                    .map(|point| point.tx_rate.max(point.rx_rate))
                    .fold(0.0, f64::max);

                let datasets = vec![
                    Dataset::default()
                        .name("TX")
                        .marker(symbols::Marker::Dot)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Green))
                        .data(&tx_data),
                    Dataset::default()
                        .name("RX")
                        .marker(symbols::Marker::Dot)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(Color::Red))
                        .data(&rx_data),
                ];
                
                let x_labels = vec![
                    Line::from("5m ago"),
                    Line::from("2.5m ago"),
                    Line::from("now"),
                ];
                
                let y_max = format!("{:.1} Mbps", max_rate);
                let y_labels = vec![
                    Line::from("0"),
                    Line::from(y_max.as_str()),
                ];

                let chart = Chart::new(datasets)
                    .block(Block::default()
                        .title("Network Throughput")
                        .borders(Borders::ALL))
                    .x_axis(
                        Axis::default()
                            .title("Time")
                            .bounds([0.0, 59.0])
                            .labels(x_labels)
                    )
                    .y_axis(
                        Axis::default()
                            .title("Mbps")
                            .bounds([0.0, max_rate * 1.1])
                            .labels(y_labels)
                    );

                f.render_widget(chart, area);
            }
        }
    }

    fn render_radio_stats(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            if let Some(interfaces) = &device.interfaces {
                let radios = interfaces.radios.as_slice();
                let radio_info: Vec<Row> = radios
                    .iter()
                    .map(|radio| {
                        let freq = radio.frequency_ghz.as_ref().map_or("Unknown", |f| match f {
                            FrequencyBand::Band2_4GHz => "2.4 GHz",
                            FrequencyBand::Band5GHz => "5 GHz",
                            FrequencyBand::Band6GHz => "6 GHz",
                            FrequencyBand::Band60GHz => "60 GHz",
                        });

                        Row::new(vec![
                            Cell::from(freq),
                            Cell::from(radio.channel.map_or("N/A".to_string(), |c| c.to_string())),
                            Cell::from(
                                radio
                                    .channel_width_mhz
                                    .map_or("N/A".to_string(), |w| format!("{} MHz", w)),
                            ),
                        ])
                    })
                    .collect();

                let header = Row::new(vec!["Frequency", "Channel", "Width"]);

                let widths = [
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                ];

                let table = Table::new(radio_info, widths).header(header).block(
                    Block::default()
                        .title("Radio Information")
                        .borders(Borders::ALL),
                );

                f.render_widget(table, area);
            }
        }
    }

    fn render_port_status(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            if let Some(interfaces) = &device.interfaces {
                let ports = interfaces.ports.as_slice();
                let port_info: Vec<Row> = ports
                    .iter()
                    .map(|port| {
                        let status_style = match port.state {
                            PortState::Up => Style::default().fg(Color::Green),
                            PortState::Down => Style::default().fg(Color::Red),
                            PortState::Unknown => Style::default().fg(Color::Yellow),
                        };

                        Row::new(vec![
                            Cell::from(port.idx.to_string()),
                            Cell::from(format!("{:?}", port.connector)),
                            Cell::from(format!("{:?}", port.state)).style(status_style),
                            Cell::from(format!("{} Mbps", port.speed_mbps)),
                        ])
                    })
                    .collect();

                let header = Row::new(vec!["Port", "Type", "Status", "Speed"]);

                let widths = [
                    Constraint::Percentage(20),
                    Constraint::Percentage(30),
                    Constraint::Percentage(20),
                    Constraint::Percentage(30),
                ];

                let table = Table::new(port_info, widths)
                    .header(header)
                    .block(Block::default().title("Port Status").borders(Borders::ALL));

                f.render_widget(table, area);
            }
        }
    }
}
