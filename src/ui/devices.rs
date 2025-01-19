use crate::app::{App, SortOrder};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;
use unifi_rs::DeviceState;

pub fn render_devices(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Summary header
            Constraint::Min(0),     // Device table
            Constraint::Length(3),  // Controls
        ])
        .split(area);

    render_device_summary(f, app, chunks[0]);
    render_device_table(f, app, chunks[1]);
    render_device_controls(f, chunks[2]);
}

fn render_device_summary(f: &mut Frame, app: &App, area: Rect) {
    let online_count = app.state.filtered_devices
        .iter()
        .filter(|d| matches!(d.state, DeviceState::Online))
        .count();

    let updating_count = app.state.filtered_devices
        .iter()
        .filter(|d| matches!(d.state, DeviceState::Updating))
        .count();

    let offline_count = app.state.filtered_devices
        .iter()
        .filter(|d| matches!(d.state, DeviceState::Offline))
        .count();

    let ap_count = app.state.filtered_devices
        .iter()
        .filter(|d| d.features.contains(&"accessPoint".to_string()))
        .count();

    let switch_count = app.state.filtered_devices
        .iter()
        .filter(|d| d.features.contains(&"switching".to_string()))
        .count();

    let summary_text = vec![
        Line::from(vec![
            Span::styled("Total: ", Style::default()),
            Span::styled(
                app.state.filtered_devices.len().to_string(),
                Style::default().add_modifier(Modifier::BOLD)
            ),
            Span::raw(" | "),
            Span::styled("Online: ", Style::default().fg(Color::Green)),
            Span::styled(
                online_count.to_string(),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            ),
            Span::raw(" | "),
            Span::styled("Updating: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                updating_count.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            ),
            Span::raw(" | "),
            Span::styled("Offline: ", Style::default().fg(Color::Red)),
            Span::styled(
                offline_count.to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            ),
        ]),
        Line::from(vec![
            Span::styled("📡 APs: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                ap_count.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            ),
            Span::raw(" | "),
            Span::styled("🔌 Switches: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                switch_count.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            ),
        ]),
    ];

    let title = match &app.state.selected_site {
        Some(site) => format!("Device Summary - {}", site.site_name),
        None => "Device Summary - All Sites".to_string(),
    };

    let summary = Paragraph::new(summary_text)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(summary, area);
}

fn get_status_style(state: &DeviceState) -> Style {
    match state {
        DeviceState::Online => Style::default().fg(Color::Green),
        DeviceState::Offline => Style::default().fg(Color::Red),
        DeviceState::Updating => Style::default().fg(Color::Yellow),
        DeviceState::PendingAdoption => Style::default().fg(Color::Blue),
        DeviceState::GettingReady => Style::default().fg(Color::Yellow),
        DeviceState::Adopting => Style::default().fg(Color::Blue),
        DeviceState::Deleting => Style::default().fg(Color::Red),
        DeviceState::ConnectionInterrupted => Style::default().fg(Color::Red),
        DeviceState::Isolated => Style::default().fg(Color::Red),
    }
}

fn get_resource_style(utilization: f64) -> Style {
    match utilization {
        u if u >= 90.0 => Style::default().fg(Color::Red),
        u if u >= 75.0 => Style::default().fg(Color::Yellow),
        u if u >= 50.0 => Style::default().fg(Color::Blue),
        _ => Style::default().fg(Color::Green),
    }
}

fn render_device_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Model").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Load").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Memory").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Network").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Firmware").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Uptime").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app.state.filtered_devices
        .iter()
        .map(|device| {
            let stats = app.state.device_stats.get(&device.id);
            let details = app.state.device_details.get(&device.id);
            
            let cpu_text = stats
                .and_then(|s| s.cpu_utilization_pct)
                .map_or("N/A".to_string(), |cpu| {
                    let sparkline = match cpu {
                        c if c >= 90.0 => "█",
                        c if c >= 75.0 => "▇",
                        c if c >= 50.0 => "▅",
                        c if c >= 25.0 => "▃",
                        _ => "▁",
                    };
                    format!("{}  {:.1}%", sparkline, cpu)
                });

            let memory_text = stats
                .and_then(|s| s.memory_utilization_pct)
                .map_or("N/A".to_string(), |mem| {
                    let sparkline = match mem {
                        m if m >= 90.0 => "█",
                        m if m >= 75.0 => "▇",
                        m if m >= 50.0 => "▅",
                        m if m >= 25.0 => "▃",
                        _ => "▁",
                    };
                    format!("{}  {:.1}%", sparkline, mem)
                });

            let network_text = stats
                .and_then(|s| s.uplink.as_ref())
                .map_or("N/A".to_string(), |u| {
                    let tx_mbps = u.tx_rate_bps as f64  / 1_000_000.0;
                    let rx_mbps = u.rx_rate_bps as f64  / 1_000_000.0;
                    format!(
                        "↑{:.1}/↓{:.1} Mb",
                        tx_mbps,
                        rx_mbps
                    )
                });

            let uptime_text = stats.map_or("N/A".to_string(), |s| {
                let hours = s.uptime_sec / 3600;
                if hours > 24 {
                    let days = hours / 24;
                    format!("{}d {}h", days, hours % 24)
                } else {
                    format!("{}h", hours)
                }
            });

            Row::new(vec![
                Cell::from(device.name.clone()),
                Cell::from(device.model.clone()),
                Cell::from(format!("{:?}", device.state)).style(get_status_style(&device.state)),
                Cell::from(cpu_text).style(
                    stats
                        .and_then(|s| s.cpu_utilization_pct)
                        .map_or(Style::default(), get_resource_style)
                ),
                Cell::from(memory_text).style(
                    stats
                        .and_then(|s| s.memory_utilization_pct)
                        .map_or(Style::default(), get_resource_style)
                ),
                Cell::from(network_text),
                Cell::from(
                    details
                        .map_or("N/A".to_string(), |d| d.firmware_version.clone())
                ),
                Cell::from(uptime_text),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(20), // Name
        Constraint::Percentage(15), // Model
        Constraint::Percentage(10), // Status
        Constraint::Percentage(10), // CPU
        Constraint::Percentage(10), // Memory
        Constraint::Percentage(15), // Network
        Constraint::Percentage(10), // Firmware
        Constraint::Percentage(10), // Uptime
    ];

    let title = match &app.state.selected_site {
        Some(site) => format!(
            "Devices - {} [{}]",
            site.site_name,
            app.state.filtered_devices.len()
        ),
        None => format!("All Devices [{}]", app.state.filtered_devices.len()),
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("➤ ");

    f.render_stateful_widget(table, area, &mut app.devices_table_state);
}

fn render_device_controls(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![
            Span::raw("↑/↓: Select  "),
            Span::raw("Enter: Details  "),
            Span::raw("s: Sort  "),
            Span::raw("/: Search  "),
            Span::raw("r: Restart  "),
            Span::raw("ESC: Back"),
        ]),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Controls"));

    f.render_widget(help, area);
}

pub async fn handle_device_input(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Down => {
            let i = match app.devices_table_state.selected() {
                Some(i) => {
                    if i >= app.state.filtered_devices.len().saturating_sub(1) {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.devices_table_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match app.devices_table_state.selected() {
                Some(i) => {
                    if i == 0 {
                        app.state.filtered_devices.len().saturating_sub(1)
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.devices_table_state.select(Some(i));
        }
        KeyCode::Enter => {
            if let Some(idx) = app.devices_table_state.selected() {
                if let Some(device) = app.state.filtered_devices.get(idx) {
                    app.select_device(Some(device.id));
                }
            }
        }
        KeyCode::Char('s') => {
            match app.device_sort_order {
                SortOrder::None => app.device_sort_order = SortOrder::Ascending,
                SortOrder::Ascending => app.device_sort_order = SortOrder::Descending,
                SortOrder::Descending => app.device_sort_order = SortOrder::None,
            }
            app.sort_devices();
        }
        KeyCode::Char('r') => {
            if let Some(idx) = app.devices_table_state.selected() {
                if let Some(device) = app.state.filtered_devices.get(idx).cloned() {
                    if let Some(site) = app.state.selected_site.clone() {
                        let device_name = device.name.clone();
                        app.dialog = Some(crate::app::Dialog {
                            title: "Confirm Device Restart".to_string(),
                            message: format!("Are you sure you want to restart {}?", device_name),
                            dialog_type: crate::app::DialogType::Confirmation,
                            callback: Some(Box::new(move |app| {
                                let client = app.state.client.clone();
                                let site_id = site.site_id;
                                tokio::spawn(async move {
                                    if let Err(e) = client.restart_device(site_id, device.id).await {
                                        eprintln!("Failed to restart device: {}", e);
                                    }
                                });
                                Ok(())
                            })),
                        });
                    }
                }
            }
        }
        KeyCode::Esc => {
            app.back_to_overview();
        }
        _ => {}
    }
    Ok(())
}