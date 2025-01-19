use chrono::{DateTime, Utc};
use crate::app::{App, SortOrder};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;
use unifi_rs::ClientOverview;

pub fn render_clients(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    let clients: Vec<Row> = app
        .state
        .filtered_clients
        .iter()
        .map(|client| {
            let (name, ip, mac, device_name, r#type, status) = match client {
                ClientOverview::Wired(c) => {
                    let device_name = app.state.devices.iter()
                        .find(|d| d.id == c.uplink_device_id)
                        .map_or("Unknown", |d| d.name.as_str());

                    (
                        c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                        c.base.ip_address.as_deref().unwrap_or("Unknown").to_string(),
                        c.mac_address.clone(),
                        device_name.to_string(),
                        Cell::from("Wired").style(Style::default().fg(Color::Blue)),
                        Cell::from("Connected").style(Style::default().fg(Color::Green)),
                    )
                },
                ClientOverview::Wireless(c) => {
                    let device_name = app.state.devices.iter()
                        .find(|d| d.id == c.uplink_device_id)
                        .map_or("Unknown", |d| d.name.as_str());

                    (
                        c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                        c.base.ip_address.as_deref().unwrap_or("Unknown").to_string(),
                        c.mac_address.clone(),
                        device_name.to_string(),
                        Cell::from("Wireless").style(Style::default().fg(Color::Yellow)),
                        Cell::from("Connected").style(Style::default().fg(Color::Green)),
                    )
                },
                _ => (
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    Cell::from("Other").style(Style::default().fg(Color::Red)),
                    Cell::from("Unknown").style(Style::default().fg(Color::Red)),
                ),
            };

            let connected_since = match client {
                ClientOverview::Wired(c) => format_duration(c.base.connected_at),
                ClientOverview::Wireless(c) => format_duration(c.base.connected_at),
                _ => "Unknown".to_string(),
            };

            Row::new(vec![
                Cell::from(name),
                Cell::from(ip),
                Cell::from(mac),
                Cell::from(device_name),
                r#type,
                Cell::from(connected_since),
                status,
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("IP").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("MAC").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Connected To").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Duration").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
        Constraint::Percentage(12),
        Constraint::Percentage(8),
    ];

    let title = match &app.state.selected_site {
        Some(site) => format!(
            "Clients - {} [{}]",
            site.site_name,
            app.state.filtered_clients.len()
        ),
        None => format!("All Clients [{}]", app.state.filtered_clients.len()),
    };

    let table = Table::new(clients, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::Gray))
        .highlight_symbol("➤ ");

    f.render_stateful_widget(table, chunks[0], &mut app.clients_table_state.clone());

    let help_text = vec![Line::from(
        "↑/↓: Select | Enter: Details | s: Sort | /: Search | ESC: Back",
    )];
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Controls"));
    f.render_widget(help, chunks[1]);
}

fn format_duration(connected_at: DateTime<Utc>) -> String {
    let duration = Utc::now().signed_duration_since(connected_at);
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;

    if hours > 24 {
        let days = hours / 24;
        format!("{}d {}h", days, hours % 24)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
pub async fn handle_client_input(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Down => {
            let i = match app.clients_table_state.selected() {
                Some(i) => {
                    if i >= app.state.filtered_clients.len().saturating_sub(1) {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.clients_table_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match app.clients_table_state.selected() {
                Some(i) => {
                    if i == 0 {
                        app.state.filtered_clients.len().saturating_sub(1)
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.clients_table_state.select(Some(i));
        }
        KeyCode::Enter => {
            if let Some(idx) = app.clients_table_state.selected() {
                if let Some(client) = app.state.filtered_clients.get(idx) {
                    let client_id = match client {
                        ClientOverview::Wired(c) => c.base.id,
                        ClientOverview::Wireless(c) => c.base.id,
                        _ => return Ok(()),
                    };
                    app.select_client(Some(client_id));
                }
            }
        }
        KeyCode::Char('s') => {
            match app.client_sort_order {
                SortOrder::None => app.client_sort_order = SortOrder::Ascending,
                SortOrder::Ascending => app.client_sort_order = SortOrder::Descending,
                SortOrder::Descending => app.client_sort_order = SortOrder::None,
            }
            app.sort_clients();
        }
        KeyCode::Esc => {
            app.back_to_overview();
        }
        _ => {}
    }
    Ok(())
}
