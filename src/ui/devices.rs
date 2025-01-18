use crate::app::{App, Dialog, DialogType, SortOrder};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use ratatui::Frame;
use unifi_rs::DeviceState;

pub fn render_devices(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec!["Name", "Model", "MAC", "IP", "State"])
        .style(Style::default()
            .add_modifier(Modifier::BOLD));

    let devices: Vec<Row> = app.state.filtered_devices
        .iter()
        .map(|device| {
            let state_style = match device.state {
                DeviceState::Online => Style::default(),
                DeviceState::Offline => Style::default(),
                _ => Style::default(),
            };

            Row::new(vec![
                Cell::from(device.name.clone()),
                Cell::from(device.model.clone()),
                Cell::from(device.mac_address.clone()).style(Style::default()),
                Cell::from(device.ip_address.clone()),
                Cell::from(format!("{:?}", device.state)).style(state_style),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
    ];
    
    let table = Table::new(devices, widths)
        .header(header)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default()))
        .row_highlight_style(Style::default()
            )
        .highlight_symbol("➤ ");

    f.render_stateful_widget(table, area, &mut app.devices_table_state);
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
