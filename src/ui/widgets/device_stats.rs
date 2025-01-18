use ratatui::{Frame, symbols};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Modifier};
use ratatui::widgets::{Block, Borders, Tabs, Paragraph, Row, Table, Cell, Gauge, Chart, Dataset, GraphType, Axis};
use ratatui::text::{Line, Span};
use uuid::Uuid;
use unifi_rs::{PortState, FrequencyBand};
use crate::state::AppState;

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
            .constraints([
                Constraint::Length(3),  // Tabs
                Constraint::Min(0),     // Content
            ].as_ref())
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
                    .constraints([
                        Constraint::Length(3),  // CPU
                        Constraint::Length(3),  // Memory
                        Constraint::Min(0),     // Features
                    ].as_ref())
                    .split(area);
                
                let cpu_usage = format!("CPU: {:.1}%", stats.cpu_utilization_pct.unwrap_or(0.0));
                let memory_usage = format!("Memory: {:.1}%", stats.memory_utilization_pct.unwrap_or(0.0));

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

                // Additional device info
                let info_text = vec![
                    Line::from(vec![
                        Span::raw("Firmware: "),
                        Span::styled(&device.firmware_version, Style::default().add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::raw("Uptime: "),
                        Span::styled(format!("{} hours", stats.uptime_sec / 3600), Style::default().add_modifier(Modifier::BOLD)),
                    ]),
                ];

                let info = Paragraph::new(info_text)
                    .block(Block::default().borders(Borders::ALL).title("Device Info"));
                f.render_widget(info, chunks[2]);
            }
        }
    }

    fn render_network_stats(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(stats) = app_state.device_stats.get(&self.device_id) {
            if let Some(uplink) = &stats.uplink {
                let tx_mbps = uplink.tx_rate_bps as f64 / 1_000_000.0;
                let rx_mbps = uplink.rx_rate_bps as f64 / 1_000_000.0;
                
                let tx_data = vec![(0.0, tx_mbps)];
                let rx_data = vec![(0.0, rx_mbps)];

                let dataset = vec![
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

                let chart = Chart::new(dataset)
                    .block(Block::default().title("Network Throughput").borders(Borders::ALL))
                    .x_axis(Axis::default().title("Time").bounds([0.0, 60.0]))
                    .y_axis(Axis::default().title("Mbps").bounds([0.0, 1000.0]));

                f.render_widget(chart, area);
            }
        }
    }

    fn render_radio_stats(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            if let Some(interfaces) = &device.interfaces {
                let radios = interfaces.radios.as_slice();
                let radio_info: Vec<Row> = radios.iter().map(|radio| {
                    let freq = radio.frequency_ghz.as_ref().map_or("Unknown", |f| match f {
                        FrequencyBand::Band2_4GHz => "2.4 GHz",
                        FrequencyBand::Band5GHz => "5 GHz",
                        FrequencyBand::Band6GHz => "6 GHz",
                        FrequencyBand::Band60GHz => "60 GHz",
                    });

                    Row::new(vec![
                        Cell::from(freq),
                        Cell::from(radio.channel.map_or("N/A".to_string(), |c| c.to_string())),
                        Cell::from(radio.channel_width_mhz.map_or("N/A".to_string(), |w| format!("{} MHz", w))),
                    ])
                }).collect();

                let header = Row::new(vec![
                    "Frequency",
                    "Channel",
                    "Width",
                ]);
                
                let widths = [
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                ];

                let table = Table::new(radio_info, widths)
                    .header(header)
                    .block(Block::default().title("Radio Information").borders(Borders::ALL));

                f.render_widget(table, area);
            }
        }
    }

    fn render_port_status(&self, f: &mut Frame, area: Rect, app_state: &AppState) {
        if let Some(device) = app_state.device_details.get(&self.device_id) {
            if let Some(interfaces) = &device.interfaces {
                let ports = interfaces.ports.as_slice();
                let port_info: Vec<Row> = ports.iter().map(|port| {
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
                }).collect();

                let header = Row::new(vec![
                    "Port",
                    "Type",
                    "Status",
                    "Speed",
                ]);

                
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