use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use crate::{App, SortOrder};

pub fn render_clients(f: &mut Frame, area: Rect, app: &App) {
    let selected_style = Style::default().bg(Color::Gray);

    let clients: Vec<Row> = app
        .filtered_clients
        .iter()
        .enumerate()
        .map(|(idx, client)| {
            let style = if Some(idx) == app.selected_client_index {
                selected_style
            } else {
                Style::default()
            };

            let (name, ip, mac, r#type, connected_since, status) = match client {
                unifi_rs::ClientOverview::Wired(c) => (
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
                unifi_rs::ClientOverview::Wireless(c) => (
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
            .style(style)
        })
        .collect();

    let header_cells = vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("IP").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("MAC").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Connected Since").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
    ];

    let widths = [
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(10),
    ];

    let table = Table::new(clients, widths)
        .header(Row::new(header_cells))
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Clients ({}) [{}]",
            app.filtered_clients.len(),
            match app.client_sort_order {
                SortOrder::Ascending => "↑",
                SortOrder::Descending => "↓",
                SortOrder::None => "-",
            }
        )))
        .row_highlight_style(selected_style)
        .highlight_symbol("> ");

    f.render_widget(table, area);
}

pub fn draw_client_detail(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    let selected_client = app
        .selected_client_index
        .and_then(|idx| app.filtered_clients.get(idx));

    if let Some(client) = selected_client {
        let details_text = match client {
            unifi_rs::ClientOverview::Wired(c) => vec![
                Line::from(vec![
                    Span::raw("Name: "),
                    Span::styled(
                        c.base.name.as_deref().unwrap_or("Unnamed"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Type: "),
                    Span::styled("Wired", Style::default().fg(Color::Blue)),
                ]),
                Line::from(vec![
                    Span::raw("MAC: "),
                    Span::styled(
                        &c.mac_address,
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("IP: "),
                    Span::styled(
                        c.base.ip_address.as_deref().unwrap_or("Unknown"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Connected Since: "),
                    Span::styled(
                        c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                        Style::default(),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Uplink Device: "),
                    Span::styled(c.uplink_device_id.to_string(), Style::default()),
                ]),
            ],
            unifi_rs::ClientOverview::Wireless(c) => vec![
                Line::from(vec![
                    Span::raw("Name: "),
                    Span::styled(
                        c.base.name.as_deref().unwrap_or("Unnamed"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Type: "),
                    Span::styled("Wireless", Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::raw("MAC: "),
                    Span::styled(
                        &c.mac_address,
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("IP: "),
                    Span::styled(
                        c.base.ip_address.as_deref().unwrap_or("Unknown"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Connected Since: "),
                    Span::styled(
                        c.base.connected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                        Style::default(),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("Uplink Device: "),
                    Span::styled(c.uplink_device_id.to_string(), Style::default()),
                ]),
            ],
            _ => vec![Line::from("Unknown client type")],
        };

        let content = Paragraph::new(details_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Client Details"),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(content, chunks[0]);
    }

    let help_text = vec![Line::from("ESC: Back | q: Quit")];
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
    f.render_widget(help, chunks[1]);
}