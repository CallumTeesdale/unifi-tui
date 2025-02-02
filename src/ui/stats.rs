use crate::app::App;
use crate::state::NetworkStats;
use crate::ui::widgets::format_network_speed;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Axis, Block, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table,
};
use ratatui::{symbols, Frame};
use unifi_rs::device::DeviceState;
use unifi_rs::models::client::ClientOverview;

pub fn render_stats(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(10), // Summary + Device Stats Table
                Constraint::Min(0),     // Network Graphs
            ]
            .as_ref(),
        )
        .split(area);

    render_summary_and_device_table(f, app, chunks[0]);
    render_network_graphs(f, app, chunks[1]);
}

fn render_summary_and_device_table(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(30), // Summary
                Constraint::Percentage(70), // Device Table
            ]
            .as_ref(),
        )
        .split(area);

    render_summary(f, app, chunks[0]);
    render_device_table(f, app, chunks[1]);
}

fn render_summary(f: &mut Frame, app: &App, area: Rect) {
    let online_devices = app
        .state
        .devices
        .iter()
        .filter(|d| matches!(d.state, DeviceState::Online))
        .count();

    let wireless_clients = app
        .state
        .clients
        .iter()
        .filter(|c| matches!(c, ClientOverview::Wireless(_)))
        .count();

    let wired_clients = app
        .state
        .clients
        .iter()
        .filter(|c| matches!(c, ClientOverview::Wired(_)))
        .count();

    let total_tx = app
        .state
        .device_stats
        .values()
        .filter_map(|stats| stats.uplink.as_ref().map(|u| u.tx_rate_bps))
        .sum::<i64>();

    let total_rx = app
        .state
        .device_stats
        .values()
        .filter_map(|stats| stats.uplink.as_ref().map(|u| u.rx_rate_bps))
        .sum::<i64>();

    let summary_text = vec![
        Line::from(format!(
            "Devices Online: {}/{}",
            online_devices,
            app.state.devices.len()
        )),
        Line::from(format!("Total Clients: {}", app.state.clients.len())),
        Line::from(format!("• Wireless: {}", wireless_clients)),
        Line::from(format!("• Wired: {}", wired_clients)),
        Line::from(""),
        Line::from("Network Link Speed:"),
        Line::from(format!("↑ {}", format_network_speed(total_tx))),
        Line::from(format!("↓ {}", format_network_speed(total_rx))),
    ];

    let title = match &app.state.selected_site {
        Some(site) => format!("Summary - {}", site.site_name),
        None => "Summary - All Sites".to_string(),
    };

    let summary =
        Paragraph::new(summary_text).block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(summary, area);
}

fn render_device_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["Device", "CPU", "Memory", "Traffic"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .state
        .devices
        .iter()
        .filter_map(|device| {
            let stats = app.state.device_stats.get(&device.id)?;
            let details = app.state.device_details.get(&device.id)?;

            let traffic = stats.uplink.as_ref().map_or("N/A".to_string(), |u| {
                format!(
                    "↑{}/↓{}",
                    format_network_speed(u.tx_rate_bps),
                    format_network_speed(u.rx_rate_bps)
                )
            });

            let style = match device.state {
                DeviceState::Online => Style::default().fg(Color::Green),
                DeviceState::Offline => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::Yellow),
            };

            Some(
                Row::new(vec![
                    Cell::from(details.name.clone()),
                    Cell::from(format!("{:.1}%", stats.cpu_utilization_pct.unwrap_or(0.0))),
                    Cell::from(format!(
                        "{:.1}%",
                        stats.memory_utilization_pct.unwrap_or(0.0)
                    )),
                    Cell::from(traffic),
                ])
                .style(style),
            )
        })
        .collect();

    let widths = [
        Constraint::Percentage(40),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(30),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Device Status"),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(table, area);
}

fn render_network_graphs(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50), // Client History
                Constraint::Percentage(50), // Network Throughput
            ]
            .as_ref(),
        )
        .split(area);

    render_client_history(f, app, chunks[0]);
    render_network_throughput(f, app, chunks[1]);
}
fn render_client_history(f: &mut Frame, app: &App, area: Rect) {
    let client_history: Vec<&NetworkStats> = app.state.stats_history.iter().collect();
    if client_history.is_empty() {
        return;
    }

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

    let max_y_label = format!("{}", max_y as i32);
    let y_axis_labels = vec![Line::from("0"), Line::from(max_y_label.as_str())];

    let x_axis_labels = vec![Line::from("5m ago"), Line::from("Now")];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title("Client History")
                .borders(Borders::ALL)
                .border_style(Style::default()),
        )
        .x_axis(
            Axis::default()
                .title("Time")
                .style(Style::default())
                .bounds([0.0, (client_history.len() - 1) as f64])
                .labels(x_axis_labels),
        )
        .y_axis(
            Axis::default()
                .title("Clients")
                .style(Style::default())
                .bounds([0.0, max_y * 1.1])
                .labels(y_axis_labels),
        );

    f.render_widget(chart, area);
}

fn render_network_throughput(f: &mut Frame, app: &App, area: Rect) {
    let stats_history: Vec<&NetworkStats> = app.state.stats_history.iter().collect();
    if stats_history.is_empty() {
        return;
    }

    let tx_data: Vec<(f64, f64)> = stats_history
        .iter()
        .enumerate()
        .map(|(i, stats)| {
            let total_tx: f64 = stats
                .device_stats
                .iter()
                .filter_map(|m| m.tx_rate)
                .sum::<i64>() as f64;
            (i as f64, total_tx)
        })
        .collect();

    let rx_data: Vec<(f64, f64)> = stats_history
        .iter()
        .enumerate()
        .map(|(i, stats)| {
            let total_rx: f64 = stats
                .device_stats
                .iter()
                .filter_map(|m| m.rx_rate)
                .sum::<i64>() as f64;
            (i as f64, total_rx)
        })
        .collect();

    let max_throughput = tx_data
        .iter()
        .chain(rx_data.iter())
        .map(|(_, rate)| *rate)
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
            .style(Style::default().fg(Color::Blue))
            .data(&rx_data),
    ];

    let max_label = format_network_speed(max_throughput as i64).to_string();
    let y_labels = vec![Line::from("0"), Line::from(max_label.as_str())];

    let x_labels = vec![
        Line::from("5m ago"),
        Line::from("2.5m ago"),
        Line::from("now"),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title("Network Link Speed (All Devices)")
                .borders(Borders::ALL)
                .border_style(Style::default()),
        )
        .x_axis(
            Axis::default()
                .title("Time")
                .style(Style::default())
                .bounds([0.0, (stats_history.len() - 1) as f64])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("Speed")
                .style(Style::default())
                .bounds([0.0, max_throughput * 1.1])
                .labels(y_labels),
        );

    f.render_widget(chart, area);
}
