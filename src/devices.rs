use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use crossterm::event::{KeyCode, KeyEvent};
use std::io;
use crate::{App, Dialog, DialogType, Mode, SortOrder};

pub fn render_devices(f: &mut Frame, area: Rect, app: &App) {
    let selected_style = Style::default().bg(Color::Gray);

    let devices: Vec<Row> = app
        .filtered_devices
        .iter()
        .enumerate()
        .map(|(idx, device)| {
            let style = if Some(idx) == app.selected_device_index {
                selected_style
            } else {
                Style::default()
            };

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
            Row::new(cells).style(style)
        })
        .collect();

    let header_cells = vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Model").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("MAC").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("IP").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("State").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Features").style(Style::default().add_modifier(Modifier::BOLD)),
    ];

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
    ];

    let table = Table::new(devices, widths)
        .header(Row::new(header_cells))
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Devices ({}) [{}]",
            app.filtered_devices.len(),
            match app.device_sort_order {
                SortOrder::Ascending => "↑",
                SortOrder::Descending => "↓",
                SortOrder::None => "-",
            }
        )))
        .row_highlight_style(selected_style)
        .highlight_symbol("> ");

    f.render_widget(table, area);
}

pub fn draw_device_detail(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    let selected_device = app
        .selected_device_index
        .and_then(|idx| app.filtered_devices.get(idx));

    if let Some(device) = selected_device {
        let state_style = match device.state {
            unifi_rs::DeviceState::Online => Style::default().fg(Color::Green),
            unifi_rs::DeviceState::Offline => Style::default().fg(Color::Red),
            _ => Style::default().fg(Color::Yellow),
        };

        let mut details_text = vec![
            Line::from(vec![
                Span::raw("Name: "),
                Span::styled(&device.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Model: "),
                Span::styled(&device.model, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("MAC: "),
                Span::styled(
                    &device.mac_address,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("IP: "),
                Span::styled(
                    &device.ip_address,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("State: "),
                Span::styled(format!("{:?}", device.state), state_style),
            ]),
            Line::from(vec![
                Span::raw("Features: "),
                Span::styled(device.features.join(", "), Style::default()),
            ]),
            Line::from(""),
        ];

        if let Some(stats) = &app.device_stats {
            details_text.extend(vec![
                Line::from(format!("Uptime: {} hours", stats.uptime_sec / 3600)),
                Line::from(format!(
                    "CPU: {}%",
                    stats.cpu_utilization_pct.unwrap_or(0.0)
                )),
                Line::from(format!(
                    "Memory: {}%",
                    stats.memory_utilization_pct.unwrap_or(0.0)
                )),
            ]);
        }

        let content = Paragraph::new(details_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Device Details"),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(content, chunks[0]);
    }

    let help_text = vec![Line::from("ESC: Back | r: Restart Device | q: Quit")];
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
    f.render_widget(help, chunks[1]);
}

pub async fn handle_device_detail_input(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Overview;
            app.device_details = None;
            app.device_stats = None;
        }
        KeyCode::Char('r') => {
            if let Some(idx) = app.selected_device_index {
                if let Some(device) = app.devices.get(idx) {
                    if let Some(site) = app.sites.first() {
                        let device_id = device.id;
                        let site_id = site.id;
                        let client = app.client.clone();

                        app.dialog = Some(Dialog {
                            title: "Confirm Restart".to_string(),
                            message: format!(
                                "Are you sure you want to restart {}? (y/n)",
                                device.name
                            ),
                            dialog_type: DialogType::Confirmation,
                            callback: Some(Box::new(move |_app| {
                                let result = tokio::runtime::Handle::current().block_on(async {
                                    client.restart_device(site_id, device_id).await
                                });
                                result.map_err(anyhow::Error::from)
                            })),
                        });
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}