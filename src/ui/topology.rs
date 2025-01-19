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
use std::string;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct NetworkNode {
    id: Uuid,
    name: String,
    device_type: String,
    x: f64,
    y: f64,
    state: DeviceState,
    uplink_to: Option<Uuid>,
    clients: Vec<ClientNode>,
}

#[derive(Debug, Clone)]
struct ClientNode {
    id: Uuid,
    name: String,
    connection_type: String, // "Wired" or "Wireless"
    x: f64,
    y: f64,
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
    let total_clients = app.state.clients.len();

    let title = match &app.state.selected_site {
        Some(site) => format!(
            "Network Topology - {} [{} devices online, {} clients]",
            site.site_name,
            online_devices,
            total_clients
        ),
        None => format!(
            "Network Topology - All Sites [{} devices online, {} clients]",
            online_devices,
            total_clients
        ),
    };

    let header = Paragraph::new(Line::from(title))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![Line::from(
        "Enter: View details | Tab: Next tab | Esc: Back | Arrow keys: Navigate",
    )];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Controls"));

    f.render_widget(help, area);
}

fn render_graph(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let inner_area = block.inner(area);

    // Create network nodes with their clients
    let nodes = create_network_nodes(app);

    // Layout nodes and their clients
    let mut laid_out_nodes = nodes.clone();
    layout_nodes(&mut laid_out_nodes, inner_area.width as f64, inner_area.height as f64);

    let canvas = Canvas::default()
        .block(block)
        .x_bounds([0.0, inner_area.width as f64])
        .y_bounds([0.0, inner_area.height as f64])
        .paint(move |ctx| {
            // Pass a clone of nodes to the drawing function
            draw_network_topology(ctx, laid_out_nodes.clone());
        });

    f.render_widget(canvas, area);
}
fn draw_network_topology(
    ctx: &mut ratatui::widgets::canvas::Context,
    nodes: Vec<NetworkNode>
) {
    // Draw connections between devices
    for node in &nodes {
        if let Some(uplink_id) = node.uplink_to {
            if let Some(uplink) = nodes.iter().find(|n| n.id == uplink_id) {
                draw_dotted_line(ctx, node.x, node.y, uplink.x, uplink.y, Color::Gray);
            }
        }
    }

    // Draw client connections and clients
    for node in &nodes {
        // Draw clients
        for client in &node.clients {
            // Draw connection line from client to device
            draw_dotted_line(ctx, client.x, client.y, node.x, node.y, Color::LightBlue);

            // Draw client
            draw_client(ctx, client.clone());
        }
    }

    // Draw devices last so they're on top
    for node in &nodes {
        draw_device(ctx, node.clone());
    }
}

fn draw_client(
    ctx: &mut ratatui::widgets::canvas::Context,
    client: ClientNode
) {
    let color = if client.connection_type == "Wireless" {
        Color::LightCyan
    } else {
        Color::LightGreen
    };

    let icon_size = 1.5;

    match client.connection_type.as_str() {
        "Wireless" => {
            // Draw a small laptop/device symbol
            let points = [
                (client.x - icon_size, client.y - icon_size/2.0),
                (client.x + icon_size, client.y - icon_size/2.0),
                (client.x + icon_size, client.y + icon_size/2.0),
                (client.x - icon_size, client.y + icon_size/2.0),
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

            // Draw small wifi symbol
            let radius = icon_size * 0.5;
            let arc_points: Vec<(f64, f64)> = (0..8).map(|j| {
                let angle = (j as f64) * std::f64::consts::PI / 7.0;
                (client.x + angle.cos() * radius, client.y - icon_size/2.0 + angle.sin() * radius)
            }).collect();
            ctx.draw(&Points {
                coords: &arc_points,
                color,
            });
        },
        "Wired" => {
            // Draw a desktop-like symbol
            let monitor_points = [
                (client.x - icon_size, client.y - icon_size/2.0),
                (client.x + icon_size, client.y - icon_size/2.0),
                (client.x + icon_size, client.y + icon_size/2.0),
                (client.x - icon_size, client.y + icon_size/2.0),
            ];

            for i in 0..monitor_points.len() {
                ctx.draw(&CanvasLine {
                    x1: monitor_points[i].0,
                    y1: monitor_points[i].1,
                    x2: monitor_points[(i + 1) % monitor_points.len()].0,
                    y2: monitor_points[(i + 1) % monitor_points.len()].1,
                    color,
                });
            }

            // Draw base
            ctx.draw(&CanvasLine {
                x1: client.x - icon_size/2.0,
                y1: client.y + icon_size/2.0,
                x2: client.x + icon_size/2.0,
                y2: client.y + icon_size/2.0,
                color,
            });
        },
        _ => {
            // Generic client symbol
            let circle_points: Vec<(f64, f64)> = (0..12).map(|i| {
                let angle = (i as f64) * 2.0 * std::f64::consts::PI / 12.0;
                (client.x + angle.cos() * icon_size, client.y + angle.sin() * icon_size)
            }).collect();
            ctx.draw(&Points {
                coords: &circle_points,
                color,
            });
        }
    }

    // Use Cow to handle both string slice and owned string
    use std::borrow::Cow;
    let name: Cow<str> = if client.name.is_empty() {
        Cow::Borrowed("Unknown")
    } else {
        Cow::Owned(client.name)
    };

    let label_x = client.x - (name.len() as f64 * 0.3);
    ctx.print(label_x, client.y + icon_size + 0.5, name);
}

fn draw_device(
    ctx: &mut ratatui::widgets::canvas::Context,
    node: NetworkNode
) {
    let color = match node.state {
        DeviceState::Online => Color::Green,
        DeviceState::Offline => Color::Red,
        _ => Color::Yellow,
    };

    let icon_size = 3.0;

    match node.device_type.as_str() {
        "Access Point" => {
            // Draw WiFi-like symbol
            for i in 0..3 {
                let radius = icon_size - (i as f64 * 0.8);
                let arc_points: Vec<(f64, f64)> = (0..16).map(|j| {
                    let angle = (j as f64) * std::f64::consts::PI / 15.0;
                    (node.x + angle.cos() * radius, node.y + angle.sin() * radius)
                }).collect();
                ctx.draw(&Points {
                    coords: &arc_points,
                    color,
                });
            }
        },
        "Switch" => {
            // Draw switch with ports
            let points = [
                (node.x - icon_size, node.y - icon_size/2.0),
                (node.x + icon_size, node.y - icon_size/2.0),
                (node.x + icon_size, node.y + icon_size/2.0),
                (node.x - icon_size, node.y + icon_size/2.0),
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

            // Draw switch ports
            for i in 1..4 {
                let port_x = node.x - icon_size + (icon_size * 0.8 * i as f64);
                ctx.draw(&Points {
                    coords: &[(port_x, node.y + icon_size/2.0)],
                    color,
                });
            }
        },
        _ => {
            // Generic device
            let circle_points: Vec<(f64, f64)> = (0..16).map(|i| {
                let angle = (i as f64) * 2.0 * std::f64::consts::PI / 16.0;
                (node.x + angle.cos() * icon_size, node.y + angle.sin() * icon_size)
            }).collect();
            ctx.draw(&Points {
                coords: &circle_points,
                color,
            });
        }
    }

    // Draw device name and client count
    let label = format!("{} ({} clients)", node.name, node.clients.len());
    let label_x = node.x - (label.len() as f64 * 0.3);
    ctx.print(label_x, node.y + icon_size + 1.0, label);
}
fn draw_dotted_line(
    ctx: &mut ratatui::widgets::canvas::Context,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color: Color,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    let steps = (len / 2.0) as i32;

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = x1 + dx * t;
        let y = y1 + dy * t;
        if i % 2 == 0 {
            ctx.draw(&Points {
                coords: &[(x, y)],
                color,
            });
        }
    }
}

fn create_network_nodes(app: &App) -> Vec<NetworkNode> {
    let mut nodes = Vec::new();
    let mut device_clients: HashMap<Uuid, Vec<ClientNode>> = HashMap::new();

    // Group clients by device
    for client in &app.state.clients {
        let (device_id, client_node) = match client {
            ClientOverview::Wired(c) => {
                let node = ClientNode {
                    id: c.base.id,
                    name: c.base.name.clone().unwrap_or_default(),
                    connection_type: "Wired".to_string(),
                    x: 0.0,
                    y: 0.0,
                };
                (c.uplink_device_id, node)
            },
            ClientOverview::Wireless(c) => {
                let node = ClientNode {
                    id: c.base.id,
                    name: c.base.name.clone().unwrap_or_default(),
                    connection_type: "Wireless".to_string(),
                    x: 0.0,
                    y: 0.0,
                };
                (c.uplink_device_id, node)
            },
            _ => continue,
        };

        device_clients.entry(device_id)
            .or_insert_with(Vec::new)
            .push(client_node);
    }

    // Create nodes for each device
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
            state: device.state.clone(),
            uplink_to,
            clients: device_clients.remove(&device.id).unwrap_or_default(),
        });
    }

    nodes
}

fn layout_nodes(nodes: &mut Vec<NetworkNode>, width: f64, height: f64) {
    // Find root nodes
    let existing_ids: HashSet<_> = nodes.iter().map(|n| n.id).collect();
    let mut root_nodes: Vec<_> = nodes.iter()
        .filter(|n| n.uplink_to.is_none() || !existing_ids.contains(&n.uplink_to.unwrap()))
        .map(|n| n.id)
        .collect();
    root_nodes.sort();

    // Position root nodes at top
    for root_id in root_nodes.iter() {
        if let Some(node) = nodes.iter_mut().find(|n| n.id == *root_id) {
            node.x = width / 2.0;
            node.y = height * 0.15;

            // Layout clients for root node in a semi-circle below
            layout_clients_for_node(node, width, height, 0.2);
        }
    }

    // Find second level nodes (direct children of root nodes)
    let second_level: Vec<_> = nodes.iter()
        .filter(|n| {
            n.uplink_to.map_or(false, |up| root_nodes.contains(&up))
        })
        .map(|n| n.id)
        .collect();

    // Position second level in an arc
    let second_count = second_level.len();
    if second_count > 0 {
        let arc_start = -std::f64::consts::PI / 3.0;
        let arc_end = std::f64::consts::PI / 3.0;
        let arc_step = (arc_end - arc_start) / (second_count.max(1) - 1) as f64;

        for (i, node_id) in second_level.iter().enumerate() {
            if let Some(node) = nodes.iter_mut().find(|n| n.id == *node_id) {
                let angle = arc_start + (i as f64 * arc_step);
                let radius = height * 0.25;
                node.x = width/2.0 + radius * angle.sin();
                node.y = height * 0.4 + radius * angle.cos().abs();

                // Layout clients for this node
                layout_clients_for_node(node, width, height, 0.4);
            }
        }
    }

    // Position remaining nodes in a line at the bottom
    let remaining: Vec<_> = nodes.iter()
        .filter(|n| n.y == 0.0)
        .map(|n| n.id)
        .collect();

    let bottom_spacing = width / (remaining.len() + 1) as f64;
    for (i, node_id) in remaining.iter().enumerate() {
        if let Some(node) = nodes.iter_mut().find(|n| n.id == *node_id) {
            node.x = bottom_spacing * (i + 1) as f64;
            node.y = height * 0.7;

            // Layout clients for bottom nodes
            layout_clients_for_node(node, width, height, 0.6);
        }
    }
}

fn layout_clients_for_node(node: &mut NetworkNode, width: f64, height: f64, level_factor: f64) {
    let client_count = node.clients.len();
    if client_count == 0 {
        return;
    }

    // Arrange clients in a partial circle below their device
    let radius = height * 0.1; // Radius for client arrangement
    let arc_width = std::f64::consts::PI / 2.0; // How wide to spread the clients (in radians)

    for (i, client) in node.clients.iter_mut().enumerate() {
        let angle = -arc_width/2.0 + (i as f64 * arc_width / (client_count - 1).max(1) as f64);
        client.x = node.x + radius * angle.sin();
        client.y = node.y + radius * angle.cos().abs() + height * 0.05;
    }
}