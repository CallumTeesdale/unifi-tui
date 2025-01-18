use crate::app::{App};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::Line;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

pub fn render_sites(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    let sites: Vec<Row> = app
        .state
        .sites
        .iter()
        .map(|site| {
            let is_selected = app
                .state
                .selected_site
                .as_ref()
                .is_some_and(|s| s.site_id == site.id);

            let style = if is_selected {
                Style::default().bg(Color::Gray)
            } else {
                Style::default()
            };

            let cells = vec![
                Cell::from(site.id.to_string()),
                Cell::from(site.name.as_deref().unwrap_or("Unnamed")),
            ];
            Row::new(cells).style(style)
        })
        .collect();

    let is_selected = app.state.selected_site.is_none();
    
    let style = if is_selected {
        Style::default()
    } else {
        Style::default()
    };

    let header = Row::new(vec![
        Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let widths = [Constraint::Percentage(30), Constraint::Percentage(70)];

    let table = Table::new(sites, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Sites"))
        .row_highlight_style(Style::default().bg(Color::Gray));

    f.render_stateful_widget(table, chunks[0], &mut app.sites_table_state.clone());
    
    let help_text = vec![Line::from(
        "↑/↓: Select site | Enter: View site | Esc: Show all sites",
    )];
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Quick Help"));
    f.render_widget(help, chunks[1]);
}

pub fn handle_sites_input(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Down => {
            let i = match app.sites_table_state.selected() {
                Some(i) => {
                    if i >= app.state.sites.len().saturating_sub(1) {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            app.sites_table_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match app.sites_table_state.selected() {
                Some(i) => {
                    if i == 0 {
                        app.state.sites.len().saturating_sub(1)
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            app.sites_table_state.select(Some(i));
        }
        KeyCode::Enter => {
            if let Some(idx) = app.sites_table_state.selected() {
                if let Some(site) = app.state.sites.get(idx) {
                    app.state.set_site_context(Some(site.id));
                }
            }
        }
        KeyCode::Esc => {
            app.sites_table_state.select(None);
            app.state.set_site_context(None);
        }
        _ => {}
    }
    Ok(())
}
