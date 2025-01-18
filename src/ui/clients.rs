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
            let (name, ip, mac, r#type, connected_since, status) = match client {
                ClientOverview::Wired(c) => (
                    c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                    c.base
                        .ip_address
                        .as_deref()
                        .unwrap_or("Unknown")
                        .to_string(),
                    c.mac_address.clone(),
                    Cell::from("Wired").style(Style::default().fg(Color::Blue)),
                    c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Cell::from("Connected").style(Style::default().fg(Color::Green)),
                ),
                ClientOverview::Wireless(c) => (
                    c.base.name.as_deref().unwrap_or("Unnamed").to_string(),
                    c.base
                        .ip_address
                        .as_deref()
                        .unwrap_or("Unknown")
                        .to_string(),
                    c.mac_address.clone(),
                    Cell::from("Wireless").style(Style::default().fg(Color::Yellow)),
                    c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Cell::from("Connected").style(Style::default().fg(Color::Green)),
                ),
                _ => (
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    "Unknown".to_string(),
                    Cell::from("Other").style(Style::default().fg(Color::Red)),
                    "Unknown".to_string(),
                    Cell::from("Unknown").style(Style::default().fg(Color::Red)),
                ),
            };

            Row::new(vec![
                Cell::from(name),
                Cell::from(ip),
                Cell::from(mac),
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
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Connected Since").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
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
        .row_highlight_style(Style::default().bg(Color::Gray))
        .highlight_symbol("> ")
        .widths(widths);

    f.render_stateful_widget(table, chunks[0], &mut app.clients_table_state.clone());

    let help_text = vec![Line::from(
        "↑/↓: Select | Enter: Details | s: Sort | /: Search",
    )];
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
    f.render_widget(help, chunks[1]);
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
