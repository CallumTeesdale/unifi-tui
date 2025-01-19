use crate::app::{App, Dialog, DialogType, SortOrder};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;
use ratatui::prelude::{Color, Direction, Layout, Line};
use unifi_rs::DeviceState;

pub fn render_devices(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),        // Main device table
            Constraint::Length(3),     // Help bar
        ].as_ref())
        .split(area);

    let header = Row::new(vec![
        "Name",
        "Model",
        "Status",
        "Load",
        "Memory",
        "Network",
        "Uptime",
    ]).style(Style::default().add_modifier(Modifier::BOLD));

    let devices: Vec<Row> = app
        .state
        .filtered_devices
        .iter()
        .map(|device| {
            let stats = app.state.device_stats.get(&device.id);
            let details = app.state.device_details.get(&device.id);
            
            let status_style = match device.state {
                DeviceState::Online => Style::default().fg(Color::Green),
                DeviceState::Offline => Style::default().fg(Color::Red),
                DeviceState::Updating => Style::default().fg(Color::Yellow),
                _ => Style::default().fg(Color::Gray),
            };

            let status_text = match &device.state {
                DeviceState::Online => "Online",
                DeviceState::Offline => "Offline",
                DeviceState::PendingAdoption => "Pending",
                DeviceState::Updating => "Updating",
                DeviceState::GettingReady => "Starting",
                DeviceState::Adopting => "Adopting",
                DeviceState::Deleting => "Deleting",
                DeviceState::ConnectionInterrupted => "Interrupted",
                DeviceState::Isolated => "Isolated",
            };
            
            let cpu_text = stats
                .and_then(|s| s.cpu_utilization_pct)
                .map_or("N/A".to_string(), |cpu| format!("{:.1}%", cpu));

            let cpu_style = stats
                .and_then(|s| s.cpu_utilization_pct)
                .map_or(Style::default(), |cpu| {
                    if cpu > 80.0 {
                        Style::default().fg(Color::Red)
                    } else if cpu > 60.0 {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Green)
                    }
                });
            
            let memory_text = stats
                .and_then(|s| s.memory_utilization_pct)
                .map_or("N/A".to_string(), |mem| format!("{:.1}%", mem));

            let memory_style = stats
                .and_then(|s| s.memory_utilization_pct)
                .map_or(Style::default(), |mem| {
                    if mem > 80.0 {
                        Style::default().fg(Color::Red)
                    } else if mem > 60.0 {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Green)
                    }
                });
            
            let network_text = stats
                .and_then(|s| s.uplink.as_ref())
                .map_or("N/A".to_string(), |u| {
                    format!("↑{:.1}/↓{:.1} Mb",
                            u.tx_rate_bps as f64 / 1_000_000.0,
                            u.rx_rate_bps as f64 / 1_000_000.0
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
                Cell::from(status_text).style(status_style),
                Cell::from(cpu_text).style(cpu_style),
                Cell::from(memory_text).style(memory_style),
                Cell::from(network_text),
                Cell::from(uptime_text),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(25),  // Name
        Constraint::Percentage(15),  // Model
        Constraint::Percentage(10),  // Status
        Constraint::Percentage(10),  // CPU
        Constraint::Percentage(10),  // Memory
        Constraint::Percentage(20),  // Network
        Constraint::Percentage(10),  // Uptime
    ];

    let title = match &app.state.selected_site {
        Some(site) => format!(
            "Devices - {} [{}]",
            site.site_name,
            app.state.filtered_devices.len()
        ),
        None => format!("All Devices [{}]", app.state.filtered_devices.len()),
    };

    let table = Table::new(devices, widths)
        .header(header)
        .widths(&widths)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(Color::Gray))
        .highlight_symbol("➤ ");

    f.render_stateful_widget(table, chunks[0], &mut app.devices_table_state);
    
    let help_text = vec![Line::from(
        "↑/↓: Select | Enter: Details | r: Restart | s: Sort | /: Search | ESC: Back"
    )];
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Controls"));
    f.render_widget(help, chunks[1]);
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
        KeyCode::Char('r') => {
            if let Some(idx) = app.devices_table_state.selected() {
                if let Some(device) = app.state.filtered_devices.get(idx) {
                    if let Some(site) = app.state.selected_site.as_ref() {
                        let device_id = device.id;
                        let site_id = site.site_id;
                        let client = app.state.client.clone();

                        // Create the dialog
                        let dialog = Dialog {
                            title: "Confirm Restart".to_string(),
                            message: format!(
                                "Are you sure you want to restart {}? (y/n)",
                                device.name
                            ),
                            dialog_type: DialogType::Confirmation,
                            callback: Some(Box::new(move |_app| {
                                tokio::runtime::Handle::current()
                                    .block_on(async {
                                        client.restart_device(site_id, device_id).await
                                    })
                                    .map_err(anyhow::Error::from)
                            })),
                        };

                        app.state.set_error(format!("Setting dialog for device: {}", device.name));
                        app.dialog = Some(dialog);
                    }
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
        KeyCode::Esc => {
            app.back_to_overview();
        }
        _ => {}
    }
    Ok(())
}
