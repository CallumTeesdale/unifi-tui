use crate::app::App;
use crate::state::{DeviceMetrics, NetworkStats};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph};
use ratatui::{symbols, Frame};

pub fn render_stats(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(7), // Summary
                Constraint::Min(0),    // Charts
            ]
            .as_ref(),
        )
        .split(area);

    render_summary(f, app, chunks[0]);
    render_charts(f, app, chunks[1]);
}

fn render_summary(f: &mut Frame, app: &App, area: Rect) {
    let online_devices = app
        .state
        .devices
        .iter()
        .filter(|d| matches!(d.state, unifi_rs::DeviceState::Online))
        .count();

    let summary_text = vec![
        Line::from(format!(
            "Total Devices: {} ({} online)",
            app.state.devices.len(),
            online_devices
        )),
        Line::from(format!("Total Clients: {}", app.state.clients.len())),
        Line::from(format!(
            "Wireless Clients: {}",
            app.state
                .clients
                .iter()
                .filter(|c| matches!(c, unifi_rs::ClientOverview::Wireless(_)))
                .count()
        )),
        Line::from(format!(
            "Wired Clients: {}",
            app.state
                .clients
                .iter()
                .filter(|c| matches!(c, unifi_rs::ClientOverview::Wired(_)))
                .count()
        )),
    ];

    let title = match &app.state.selected_site {
        Some(site) => format!("Network Summary - {}", site.site_name),
        None => "Network Summary - All Sites".to_string(),
    };

    let summary =
        Paragraph::new(summary_text).block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(summary, area);
}

fn render_charts(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(area);

    render_client_history(f, app, chunks[0]);
    render_device_metrics(f, app, chunks[1]);
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

fn render_device_metrics(f: &mut Frame, app: &App, area: Rect) {
    let device_metrics: Vec<&DeviceMetrics> = app
        .state
        .stats_history
        .back()
        .map(|stats| stats.device_stats.as_slice())
        .unwrap_or(&[])
        .iter()
        .collect();

    if device_metrics.is_empty() {
        return;
    }

    let cpu_data: Vec<(f64, f64)> = device_metrics
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.cpu_utilization.map(|cpu| (i as f64, cpu)))
        .collect();

    let memory_data: Vec<(f64, f64)> = device_metrics
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.memory_utilization.map(|mem| (i as f64, mem)))
        .collect();

    let datasets = vec![
        Dataset::default()
            .name("CPU")
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Red))
            .data(&cpu_data),
        Dataset::default()
            .name("Memory")
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Blue))
            .data(&memory_data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title("Device Metrics")
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title("Device")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, (device_metrics.len() - 1) as f64]),
        )
        .y_axis(
            Axis::default()
                .title("Utilization %")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, 100.0]),
        );

    f.render_widget(chart, area);
}
