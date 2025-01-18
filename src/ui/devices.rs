use crate::app::{App, Dialog, DialogType, SortOrder};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

pub fn render_devices(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    let devices: Vec<Row> = app
        .state
        .filtered_devices
        .iter()
        .map(|device| {
            let state_style = match device.state {
                unifi_rs::DeviceState::Online => Style::default().fg(Color::Green),
                unifi_rs::DeviceState::Offline => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::Yellow),
            };

            let cells = vec![
                Cell::from(device.name.clone()),
                Cell::from(device.model.clone()),
                Cell::from(device.mac_address.clone()),
                Cell::from(device.ip_address.clone()),
                Cell::from(format!("{:?}", device.state)).style(state_style),
                Cell::from(device.features.join(", ")),
            ];
            Row::new(cells)
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Model").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("MAC").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("IP").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("State").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Features").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
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
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(Color::Gray))
        .highlight_symbol("> ");

    f.render_stateful_widget(table, chunks[0], &mut app.devices_table_state.clone());

    let help_text = vec![Line::from(
        "↑/↓: Select | Enter: Details | r: Restart | s: Sort | /: Search",
    )];
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
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

                        app.dialog = Some(Dialog {
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
                        });
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
