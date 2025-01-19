use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
    text::Line,
    widgets::{Block, Borders, Paragraph, canvas::{Canvas, Line as CanvasLine, Points}},
    Frame,
};
use unifi_rs::{ClientOverview, DeviceState};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct NetworkNode {
    id: Uuid,
    name: String,
    device_type: String,
    x: f64,
    y: f64,
    clients: usize,
    state: DeviceState,
    uplink_to: Option<Uuid>,
}

pub fn render_topology(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Network graph
            Constraint::Length(3),  // Help
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_graph(f, app, chunks[1]);
    render_help(f, chunks[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let online_devices = app.state.devices.iter()
        .filter(|d| matches!(d.state, DeviceState::Online))
        .count();

    let title = match &app.state.selected_site {
        Some(site) => format!(
            "Network Topology - {} [{} devices online]",
            site.site_name,
            online_devices
        ),
        None => format!("Network Topology - All Sites [{} devices online]", online_devices),
    };

    let header = Paragraph::new(Line::from(title))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![Line::from(
        "Enter: View device details | Tab: Next tab | Esc: Back",
    )];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Controls"));

    f.render_widget(help, area);
}

fn render_graph(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner_area = block.inner(area);
    
    let mut nodes = create_network_nodes(app);
    
    layout_nodes(&mut nodes, inner_area.width as f64, inner_area.height as f64);
    
    let canvas = Canvas::default()
        .block(block)
        .x_bounds([0.0, inner_area.width as f64])
        .y_bounds([0.0, inner_area.height as f64])
        .paint(|ctx| {
            // Draw connections first
            for node in &nodes {
                if let Some(uplink_id) = node.uplink_to {
                    if let Some(uplink) = nodes.iter().find(|n| n.id == uplink_id) {
                        let dx = uplink.x - node.x;
                        let dy = uplink.y - node.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        let steps = (len / 2.0) as i32;

                        for i in 0..=steps {
                            let t = i as f64 / steps as f64;
                            let x = node.x + dx * t;
                            let y = node.y + dy * t;
                            if i % 2 == 0 {
                                ctx.draw(&Points {
                                    coords: &[(x, y)],
                                    color: Color::Gray,
                                });
                            }
                        }
                    }
                }
            }
            
            for node in &nodes {
                draw_device(ctx, node);
            }
        });

    f.render_widget(canvas, area);
}

fn draw_device(ctx: &mut ratatui::widgets::canvas::Context, node: &NetworkNode) {
    let color = match node.state {
        DeviceState::Online => Color::Green,
        DeviceState::Offline => Color::Red,
        _ => Color::Yellow,
    };

    let icon_size = 3.0;
    let x = node.x;
    let y = node.y;

    match node.device_type.as_str() {
        "Access Point" => {
            for i in 0..3 {
                let radius = icon_size - (i as f64 * 0.8);
                let arc_points: Vec<(f64, f64)> = (0..16).map(|j| {
                    let angle = (j as f64) * std::f64::consts::PI / 15.0;
                    (x + angle.cos() * radius, y + angle.sin() * radius)
                }).collect();
                ctx.draw(&Points {
                    coords: &arc_points,
                    color,
                });
            }
        },
        "Switch" => {
            let points = [
                (x - icon_size, y - icon_size/2.0),
                (x + icon_size, y - icon_size/2.0),
                (x + icon_size, y + icon_size/2.0),
                (x - icon_size, y + icon_size/2.0),
            ];

            for i in 0..points.len() {
                ctx.draw(&CanvasLine {
                    x1: points[i].0,
                    y1: points[i].1,
                    x2: points[(i + 1) % points.len()].0,
                    y2: points[(i + 1) % points.len()].1,
                    color,
                });
            }
            for i in 1..4 {
                let port_x = x - icon_size + (icon_size * 0.8 * i as f64);
                ctx.draw(&Points {
                    coords: &[(port_x, y + icon_size/2.0)],
                    color,
                });
            }
        },
        _ => {
            let circle_points: Vec<(f64, f64)> = (0..16).map(|i| {
                let angle = (i as f64) * 2.0 * std::f64::consts::PI / 16.0;
                (x + angle.cos() * icon_size, y + angle.sin() * icon_size)
            }).collect();
            ctx.draw(&Points {
                coords: &circle_points,
                color,
            });
        }
    }
    
    let label = format!("{} ({} clients)", node.name, node.clients);
    let offset = label.len() as f64 * 0.3;
    ctx.print(x - offset, y + icon_size + 1.0, label);
}

fn create_network_nodes(app: &App) -> Vec<NetworkNode> {
    let mut nodes = Vec::new();
    let mut device_clients: HashMap<Uuid, usize> = HashMap::new();
    
    for client in &app.state.clients {
        if let Some(device_id) = match client {
            ClientOverview::Wired(c) => Some(c.uplink_device_id),
            ClientOverview::Wireless(c) => Some(c.uplink_device_id),
            _ => None,
        } {
            *device_clients.entry(device_id).or_insert(0) += 1;
        }
    }
    
    for device in &app.state.devices {
        let device_type = if device.features.contains(&"accessPoint".to_string()) {
            "Access Point"
        } else if device.features.contains(&"switching".to_string()) {
            "Switch"
        } else {
            "Other"
        };

        let uplink_to = if let Some(details) = app.state.device_details.get(&device.id) {
            details.uplink.as_ref().map(|u| u.device_id)
        } else {
            None
        };

        nodes.push(NetworkNode {
            id: device.id,
            name: device.name.clone(),
            device_type: device_type.to_string(),
            x: 0.0,
            y: 0.0,
            clients: device_clients.get(&device.id).copied().unwrap_or(0),
            state: device.state.clone(),
            uplink_to,
        });
    }

    nodes
}

fn layout_nodes(nodes: &mut [NetworkNode], width: f64, height: f64) {
    
    let existing_ids: HashSet<_> = nodes.iter().map(|n| n.id).collect();
    let mut root_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.uplink_to.is_none() || !existing_ids.contains(&n.uplink_to.unwrap()))
        .map(|n| n.id)
        .collect();
    root_nodes.sort();
    
    for root_id in root_nodes.iter() {
        if let Some(node) = nodes.iter_mut().find(|n| n.id == *root_id) {
            node.x = width / 2.0;
            node.y = height * 0.15;
        }
    }
    
    let second_level: Vec<_> = nodes.iter()
        .filter(|n| {
            n.uplink_to.is_some_and(|up| root_nodes.contains(&up))
        })
        .map(|n| n.id)
        .collect();
    
    let second_count = second_level.len();
    for (i, node_id) in second_level.iter().enumerate() {
        if let Some(node) = nodes.iter_mut().find(|n| n.id == *node_id) {
            let angle = -std::f64::consts::PI / 3.0 +
                (i as f64 * 2.0 * std::f64::consts::PI / 3.0 / (second_count - 1).max(1) as f64);
            let radius = height * 0.25;
            node.x = width/2.0 + radius * angle.sin();
            node.y = height * 0.4 + radius * angle.cos().abs();
        }
    }
    
    let remaining: Vec<_> = nodes.iter()
        .filter(|n| n.y == 0.0)
        .map(|n| n.id)
        .collect();

    let bottom_spacing = width / (remaining.len() + 1) as f64;
    for (i, node_id) in remaining.iter().enumerate() {
        if let Some(node) = nodes.iter_mut().find(|n| n.id == *node_id) {
            node.x = bottom_spacing * (i + 1) as f64;
            node.y = height * 0.7;
        }
    }
}