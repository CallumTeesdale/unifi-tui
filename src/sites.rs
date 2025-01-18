use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use ratatui::prelude::{Color, Style};
use crate::App;

pub fn render_sites(f: &mut Frame, area: Rect, app: &App) {
    let sites: Vec<Row> = app
        .sites
        .iter()
        .map(|site| {
            let cells = vec![
                Cell::from(site.id.to_string()),
                Cell::from(site.name.as_deref().unwrap_or("Unnamed")),
            ];
            Row::new(cells)
        })
        .collect();

    let widths = [Constraint::Percentage(30), Constraint::Percentage(70)];

    let table = Table::new(sites, widths)
        .header(Row::new(vec!["ID", "Name"]))
        .block(Block::default().borders(Borders::ALL).title("Sites"))
        .row_highlight_style(Style::default().bg(Color::Gray))
        .highlight_symbol("> ");

    f.render_widget(table, area);
}