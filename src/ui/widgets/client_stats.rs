use crate::state::AppState;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table,
};
use ratatui::{symbols, Frame};
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(area);

        if let Some(client) = self.app_state.clients.iter().find(|c| match c {
            ClientOverview::Wireless(w) => w.base.id == self.client_id,
            ClientOverview::Wired(w) => w.base.id == self.client_id,
            _ => false,
        }) {
            match client {
                ClientOverview::Wireless(wireless) => {
                    self.render_wireless_stats(f, chunks[1], wireless);
                }
                ClientOverview::Wired(wired) => {
                    self.render_wired_stats(f, chunks[1], wired);
                }
                _ => {}
            }
        }
    }

    fn render_wireless_stats(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WirelessClientOverview,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
            .split(area);

        let info_text = vec![
            Line::from(vec![
                Span::raw("Name: "),
                Span::styled(
                    client.base.name.as_deref().unwrap_or("Unnamed"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("MAC Address: "),
                Span::styled(
                    &client.mac_address,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("IP Address: "),
                Span::styled(
                    client.base.ip_address.as_deref().unwrap_or("Unknown"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("Connected Since: "),
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
                Span::raw("Uplink Device: "),
                Span::styled(client.uplink_device_id.to_string(), Style::default()),
            ]),
        ];

        let info = Paragraph::new(info_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Connection Info"),
        );
        f.render_widget(info, chunks[0]);

        let metrics_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(chunks[1]);

        let signal_info = vec![
            Row::new(vec![
                Cell::from("Signal Strength"),
                Cell::from("-65 dBm").style(Style::default().fg(Color::Green)),
            ]),
            Row::new(vec![Cell::from("Noise Floor"), Cell::from("-95 dBm")]),
            Row::new(vec![
                Cell::from("SNR"),
                Cell::from("30 dB").style(Style::default().fg(Color::Green)),
            ]),
            Row::new(vec![Cell::from("TX Rate"), Cell::from("867 Mbps")]),
            Row::new(vec![Cell::from("RX Rate"), Cell::from("867 Mbps")]),
        ];

        let width = [Constraint::Percentage(40), Constraint::Percentage(60)];
        let signal_table = Table::new(signal_info, width).block(
            Block::default()
                .borders(Borders::ALL)
                .title("WiFi Performance"),
        );

        f.render_widget(signal_table, metrics_layout[0]);

        let dataset = vec![
            Dataset::default()
                .name("TX")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green))
                .data(&[(0.0, 50.0), (1.0, 70.0), (2.0, 60.0)]),
            Dataset::default()
                .name("RX")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Red))
                .data(&[(0.0, 40.0), (1.0, 45.0), (2.0, 42.0)]),
        ];

        let chart = Chart::new(dataset)
            .block(
                Block::default()
                    .title("Network Throughput")
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, 2.0]),
            )
            .y_axis(
                Axis::default()
                    .title("Mbps")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, 100.0]),
            );

        f.render_widget(chart, metrics_layout[1]);
    }

    fn render_wired_stats(
        &self,
        f: &mut Frame,
        area: Rect,
        client: &unifi_rs::WiredClientOverview,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
            .split(area);

        let info_text = vec![
            Line::from(vec![
                Span::raw("Name: "),
                Span::styled(
                    client.base.name.as_deref().unwrap_or("Unnamed"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("MAC Address: "),
                Span::styled(
                    &client.mac_address,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("IP Address: "),
                Span::styled(
                    client.base.ip_address.as_deref().unwrap_or("Unknown"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("Connected Since: "),
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
                Span::raw("Uplink Device: "),
                Span::styled(client.uplink_device_id.to_string(), Style::default()),
            ]),
        ];

        let info = Paragraph::new(info_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Connection Info"),
        );
        f.render_widget(info, chunks[0]);

        let metrics_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(chunks[1]);

        let port_info = vec![
            Row::new(vec![Cell::from("Link Speed"), Cell::from("1 Gbps")]),
            Row::new(vec![Cell::from("Duplex"), Cell::from("Full")]),
            Row::new(vec![Cell::from("Power"), Cell::from("802.3at PoE+")]),
            Row::new(vec![Cell::from("Port"), Cell::from("eth0")]),
        ];

        let width = [Constraint::Percentage(30), Constraint::Percentage(70)];

        let port_table = Table::new(port_info, width).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Port Information"),
        );

        f.render_widget(port_table, metrics_layout[0]);

        let dataset = vec![
            Dataset::default()
                .name("TX")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Green))
                .data(&[(0.0, 100.0), (1.0, 150.0), (2.0, 120.0)]),
            Dataset::default()
                .name("RX")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Red))
                .data(&[(0.0, 80.0), (1.0, 90.0), (2.0, 85.0)]),
        ];

        let chart = Chart::new(dataset)
            .block(
                Block::default()
                    .title("Network Throughput")
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, 2.0]),
            )
            .y_axis(
                Axis::default()
                    .title("Mbps")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, 200.0]),
            );

        f.render_widget(chart, metrics_layout[1]);
    }
}
