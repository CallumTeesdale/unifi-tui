use crate::app::App;
use crate::ui::topology::node::NodeType;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::prelude::{Modifier, Style};
use ratatui::widgets::canvas::Canvas;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_topology(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Topology view
            Constraint::Length(3), // Status bar
        ])
        .split(area);

    let title = match &app.state.selected_site {
        Some(site) => format!("Network Topology - {}", site.site_name),
        None => "Network Topology - All Sites".to_string(),
    };
    let header = Paragraph::new(Line::from(title)).block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    let topology_block = Block::default()
        .borders(Borders::ALL)
        .title("Network Map")
        .style(Style::default().remove_modifier(Modifier::RAPID_BLINK));

    let canvas = Canvas::default()
        .block(topology_block)
        .x_bounds([0.0, 100.0])
        .y_bounds([0.0, 100.0])
        .marker(symbols::Marker::Braille)
        .paint(|ctx| {
            app.topology_view.render(ctx);
        });

    f.render_widget(canvas, chunks[1]);

    let selected_info = if let Some(node) = app.topology_view.get_selected_node() {
        match &node.node_type {
            NodeType::Device { device_type, state } => {
                format!("Selected: {} ({:?} - {:?})", node.name, device_type, state)
            }
            NodeType::Client { client_type } => {
                format!("Selected: {} ({:?})", node.name, client_type)
            }
        }
    } else {
        "No node selected".to_string()
    };

    let help_text = vec![Line::from(vec![
        Span::raw(selected_info),
        Span::raw(" | "),
        Span::raw("Mouse: Drag nodes | "),
        Span::raw("+/-: Zoom | "),
        Span::raw("r: Reset view | "),
        Span::raw("Enter: Focus | "),
        Span::raw("Esc: Back"),
    ])];

    let status_bar = Paragraph::new(help_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(status_bar, chunks[2]);
}

pub async fn handle_topology_input(app: &mut App, event: KeyEvent) -> anyhow::Result<()> {
    match event.code {
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.topology_view.zoom_in();
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            app.topology_view.zoom_out();
        }
        KeyCode::Char('r') => {
            app.topology_view.reset_view();
        }
        KeyCode::Enter => {
            if let Some(node) = app.topology_view.get_selected_node() {
                match node.node_type {
                    NodeType::Device { .. } => {
                        app.select_device(Some(node.id));
                    }
                    NodeType::Client { .. } => {
                        app.select_client(Some(node.id));
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

pub async fn handle_topology_mouse(
    app: &mut App,
    event: MouseEvent,
    area: Rect,
) -> anyhow::Result<()> {
    // Adjust mouse coordinates to account for borders and other UI elements
    app.topology_view.handle_mouse_event(event, area);
    Ok(())
}
