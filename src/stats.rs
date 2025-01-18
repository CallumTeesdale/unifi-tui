use ratatui::{symbols, Frame};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Line, Span, Style};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph};
use crate::{App, NetworkStats};

pub fn render_stats(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
        .split(area);

    let summary_text = vec![
        Line::from(format!("Total Sites: {}", app.sites.len())),
        Line::from(format!(
            "Total Devices: {} ({} online)",
            app.devices.len(),
            app.devices
                .iter()
                .filter(|d| matches!(d.state, unifi_rs::DeviceState::Online))
                .count()
        )),
        Line::from(format!("Total Clients: {}", app.clients.len())),
        Line::from(format!(
            "Wireless Clients: {}",
            app.clients
                .iter()
                .filter(|c| matches!(c, unifi_rs::ClientOverview::Wireless(_)))
                .count()
        )),
        Line::from(format!(
            "Wired Clients: {}",
            app.clients
                .iter()
                .filter(|c| matches!(c, unifi_rs::ClientOverview::Wired(_)))
                .count()
        )),
    ];

    let summary = Paragraph::new(summary_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Network Summary"),
        )
        .style(Style::default());
    f.render_widget(summary, chunks[0]);

    let client_history: Vec<&NetworkStats> = app.stats_history.iter().collect();
    if !client_history.is_empty() {
        let total_data: Vec<(f64, f64)> = client_history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.client_count as f64))
            .collect();

        let wireless_data: Vec<(f64, f64)> = client_history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.wireless_clients as f64))
            .collect();

        let wired_data: Vec<(f64, f64)> = client_history
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, s.wired_clients as f64))
            .collect();

        let max_y = client_history
            .iter()
            .map(|s| s.client_count as f64)
            .fold(0.0, f64::max);

        let datasets = vec![
            Dataset::default()
                .name("Total")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(&total_data),
            Dataset::default()
                .name("Wireless")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Yellow))
                .data(&wireless_data),
            Dataset::default()
                .name("Wired")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Blue))
                .data(&wired_data),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title("Client History")
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .title(Span::styled("Time", Style::default().fg(Color::Gray)))
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, (client_history.len() - 1) as f64])
                    .labels(
                        vec![
                            Span::styled("5m ago", Style::default().fg(Color::Gray)),
                            Span::styled("Now", Style::default().fg(Color::Gray)),
                        ]
                        .into_iter()
                        .map(Line::from)
                        .collect::<Vec<Line>>(),
                    ),
            )
            .y_axis(
                Axis::default()
                    .title(Span::styled("Clients", Style::default().fg(Color::Gray)))
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, max_y * 1.1])
                    .labels(
                        vec![
                            Span::styled("0", Style::default().fg(Color::Gray)),
                            Span::styled(
                                format!("{}", max_y as i32),
                                Style::default().fg(Color::Gray),
                            ),
                        ]
                        .into_iter()
                        .map(Line::from)
                        .collect::<Vec<Line>>(),
                    ),
            );

        f.render_widget(chart, chunks[1]);
    }
}