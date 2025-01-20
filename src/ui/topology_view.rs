use crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Context, Line, Points},
};
use std::collections::HashMap;
use unifi_rs::device::{DeviceDetails, DeviceOverview, DeviceState};
use unifi_rs::models::client::ClientOverview;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Device {
        device_type: DeviceType,
        state: DeviceState,
    },
    Client {
        client_type: ClientType,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceType {
    AccessPoint,
    Switch,
    Gateway,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientType {
    Wireless,
    Wired,
    Vpn,
}

#[derive(Debug, Clone)]
pub struct NetworkNode {
    pub id: Uuid,
    pub name: String,
    pub node_type: NodeType,
    pub x: f64,
    pub y: f64,
    pub parent_id: Option<Uuid>,
    pub children: Vec<Uuid>,
}

pub struct TopologyView {
    nodes: HashMap<Uuid, NetworkNode>,
    selected_node: Option<Uuid>,
    dragging_node: Option<Uuid>,
    last_mouse_pos: (u16, u16),
    viewport: (f64, f64), // (x_offset, y_offset)
    zoom: f64,
    canvas_dimensions: (f64, f64),
}

impl TopologyView {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            selected_node: None,
            dragging_node: None,
            last_mouse_pos: (0, 0),
            viewport: (0.0, 0.0),
            zoom: 1.0,
            canvas_dimensions: (100.0, 100.0),
        }
    }

    pub fn update_from_state(
        &mut self,
        devices: &[DeviceOverview],
        clients: &[ClientOverview],
        device_details: &HashMap<Uuid, DeviceDetails>,
    ) {
        self.nodes.clear();

        // Add device nodes
        for device in devices {
            let device_type = if device.features.contains(&"accessPoint".to_string()) {
                DeviceType::AccessPoint
            } else if device.features.contains(&"switching".to_string()) {
                DeviceType::Switch
            } else {
                DeviceType::Other
            };

            let parent_id = device_details.get(&device.id).and_then(|d| d.uplink.as_ref().map(|u| u.device_id));

            self.nodes.insert(
                device.id,
                NetworkNode {
                    id: device.id,
                    name: device.name.clone(),
                    node_type: NodeType::Device {
                        device_type,
                        state: device.state.clone(),
                    },
                    x: 0.0,
                    y: 0.0,
                    parent_id,
                    children: Vec::new(),
                },
            );
        }

        // Add client nodes and connect to devices
        for client in clients {
            let (id, name, client_type, parent_id) = match client {
                ClientOverview::Wireless(c) => (
                    c.base.id,
                    c.base.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                    ClientType::Wireless,
                    Some(c.uplink_device_id),
                ),
                ClientOverview::Wired(c) => (
                    c.base.id,
                    c.base.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                    ClientType::Wired,
                    Some(c.uplink_device_id),
                ),
                _ => continue,
            };

            self.nodes.insert(
                id,
                NetworkNode {
                    id,
                    name,
                    node_type: NodeType::Client { client_type },
                    x: 0.0,
                    y: 0.0,
                    parent_id,
                    children: Vec::new(),
                },
            );
        }

        // Update children lists
        let connections: Vec<(Uuid, Uuid)> = self
            .nodes
            .values()
            .filter_map(|node| node.parent_id.map(|parent_id| (parent_id, node.id)))
            .collect();

        for (parent_id, child_id) in connections {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.children.push(child_id);
            }
        }

        self.initialize_layout();
    }

    pub fn initialize_layout(&mut self) {
        // Find root nodes (devices without parents or with non-existent parents)
        let root_nodes: Vec<Uuid> = self
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none() || !self.nodes.contains_key(&n.parent_id.unwrap()))
            .map(|n| n.id)
            .collect();

        // Position root nodes at the top
        let root_spacing = self.canvas_dimensions.0 / (root_nodes.len() + 1) as f64;
        for (i, id) in root_nodes.iter().enumerate() {
            if let Some(node) = self.nodes.get_mut(id) {
                node.x = root_spacing * (i + 1) as f64;
                node.y = 20.0;
            }
        }

        // Layout children recursively
        for root_id in root_nodes {
            self.layout_children(root_id, 1);
        }
    }

    fn layout_children(&mut self, node_id: Uuid, depth: usize) {
        if let Some(node) = self.nodes.get(&node_id) {
            let children = node.children.clone();
            let child_count = children.len();

            if child_count > 0 {
                let parent_x = node.x;
                let spacing = 100.0 / (child_count + 1) as f64;
                let y = 20.0 + (depth as f64 * 20.0);

                for (i, child_id) in children.iter().enumerate() {
                    if let Some(child) = self.nodes.get_mut(child_id) {
                        child.x = parent_x - 50.0 + (spacing * (i + 1) as f64);
                        child.y = y;
                    }
                    self.layout_children(*child_id, depth + 1);
                }
            }
        }
    }

    pub fn handle_mouse_event(&mut self, event: MouseEvent, area: Rect) {
        // Account for borders and title in the area calculation
        let effective_area = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2)
        );

        // Convert mouse coordinates to canvas space, accounting for the border offset
        let canvas_x = ((event.column as i32 - effective_area.x as i32) as f64
            / effective_area.width as f64) * self.canvas_dimensions.0;
        let canvas_y = ((event.row as i32 - effective_area.y as i32) as f64
            / effective_area.height as f64) * self.canvas_dimensions.1;

        match event.kind {
            MouseEventKind::Down(_) => {
                // Find clicked node with increased hit area
                self.selected_node = self.find_node_at(canvas_x, canvas_y);
                self.dragging_node = self.selected_node;
                self.last_mouse_pos = (event.column, event.row);
            }
            MouseEventKind::Up(_) => {
                self.dragging_node = None;
            }
            MouseEventKind::Drag(_) => {
                if let Some(id) = self.dragging_node {
                    let dx = (event.column as i32 - self.last_mouse_pos.0 as i32) as f64
                        * (self.canvas_dimensions.0 / effective_area.width as f64);
                    let dy = (event.row as i32 - self.last_mouse_pos.1 as i32) as f64
                        * (self.canvas_dimensions.1 / effective_area.height as f64);

                    if let Some(node) = self.nodes.get_mut(&id) {
                        node.x = (node.x + dx).clamp(5.0, self.canvas_dimensions.0 - 5.0);
                        node.y = (node.y + dy).clamp(5.0, self.canvas_dimensions.1 - 5.0);

                        // Ensure the label stays visible
                        if node.x < node.name.len() as f64 * 0.4 {
                            node.x = node.name.len() as f64 * 0.4;
                        }
                        if node.x > self.canvas_dimensions.0 - node.name.len() as f64 * 0.4 {
                            node.x = self.canvas_dimensions.0 - node.name.len() as f64 * 0.4;
                        }
                    }
                }
                self.last_mouse_pos = (event.column, event.row);
            }
            _ => {}
        }
    }

    fn find_node_at(&self, x: f64, y: f64) -> Option<Uuid> {
        // Larger hit area for node selection
        const HIT_RADIUS: f64 = 5.0;

        self.nodes.iter().find_map(|(id, node)| {
            let dx = node.x - x;
            let dy = node.y - y;
            let distance = (dx * dx + dy * dy).sqrt();

            // Also check if click is within the label area
            let label_width = node.name.len() as f64 * 0.8;
            let label_hit = x >= (node.x - label_width/2.0)
                && x <= (node.x + label_width/2.0)
                && y >= (node.y + 2.0)
                && y <= (node.y + 4.0);

            if distance < HIT_RADIUS || label_hit {
                Some(*id)
            } else {
                None
            }
        })
    }

    pub fn render(&self, ctx: &mut Context) {
        // Draw connections first
        for node in self.nodes.values() {
            if let Some(parent_id) = node.parent_id {
                if let Some(parent) = self.nodes.get(&parent_id) {
                    let color = match node.node_type {
                        NodeType::Client { client_type: ClientType::Wireless } => Color::Yellow,
                        NodeType::Client { client_type: ClientType::Wired } => Color::Blue,
                        _ => Color::Gray,
                    };

                    ctx.draw(&Line {
                        x1: node.x,
                        y1: node.y,
                        x2: parent.x,
                        y2: parent.y,
                        color,
                    });
                }
            }
        }

        // Draw nodes
        for (id, node) in &self.nodes {
            let (shape, color) = match &node.node_type {
                NodeType::Device { device_type, state } => {
                    let base_color = match state {
                        DeviceState::Online => Color::Green,
                        DeviceState::Offline => Color::Red,
                        _ => Color::Yellow,
                    };

                    match device_type {
                        DeviceType::AccessPoint => ("ap", base_color),
                        DeviceType::Switch => ("switch", base_color),
                        DeviceType::Gateway => ("gateway", base_color),
                        DeviceType::Other => ("device", base_color),
                    }
                }
                NodeType::Client { client_type } => match client_type {
                    ClientType::Wireless => ("wireless", Color::Yellow),
                    ClientType::Wired => ("wired", Color::Blue),
                    ClientType::Vpn => ("vpn", Color::Cyan),
                },
            };

            self.draw_node(ctx, node, shape, color, Some(*id) == self.selected_node);
        }
    }

    fn draw_node(&self, ctx: &mut Context, node: &NetworkNode, shape: &str, color: Color, selected: bool) {
        let x = node.x;
        let y = node.y;
        let size = if selected { 3.0 } else { 2.0 };

        match shape {
            "ap" => {
                // Draw AP as a circle with waves
                for i in 0..3 {
                    let radius = size - (i as f64 * 0.5);
                    let points: Vec<(f64, f64)> = (0..16)
                        .map(|j| {
                            let angle = (j as f64) * std::f64::consts::PI / 8.0;
                            (x + angle.cos() * radius, y + angle.sin() * radius)
                        })
                        .collect();
                    ctx.draw(&Points { coords: &points, color });
                }
            }
            "switch" => {
                // Draw switch as a rectangle
                let points = [
                    (x - size, y - size / 2.0),
                    (x + size, y - size / 2.0),
                    (x + size, y + size / 2.0),
                    (x - size, y + size / 2.0),
                ];
                for i in 0..points.len() {
                    ctx.draw(&Line {
                        x1: points[i].0,
                        y1: points[i].1,
                        x2: points[(i + 1) % points.len()].0,
                        y2: points[(i + 1) % points.len()].1,
                        color,
                    });
                }
            }
            "wireless" => {
                // Draw wireless client as a small dot with waves
                ctx.draw(&Points {
                    coords: &[(x, y)],
                    color,
                });
                let points: Vec<(f64, f64)> = (0..8)
                    .map(|i| {
                        let angle = (i as f64) * std::f64::consts::PI / 4.0;
                        (x + angle.cos() * size * 0.8, y + angle.sin() * size * 0.8)
                    })
                    .collect();
                ctx.draw(&Points { coords: &points, color });
            }
            "wired" => {
                // Draw wired client as a small square
                let points = [
                    (x - size * 0.5, y - size * 0.5),
                    (x + size * 0.5, y - size * 0.5),
                    (x + size * 0.5, y + size * 0.5),
                    (x - size * 0.5, y + size * 0.5),
                ];
                for i in 0..points.len() {
                    ctx.draw(&Line {
                        x1: points[i].0,
                        y1: points[i].1,
                        x2: points[(i + 1) % points.len()].0,
                        y2: points[(i + 1) % points.len()].1,
                        color,
                    });
                }
            }
            _ => {
                // Default to circle for unknown types
                let points: Vec<(f64, f64)> = (0..16)
                    .map(|i| {
                        let angle = (i as f64) * std::f64::consts::PI / 8.0;
                        (x + angle.cos() * size, y + angle.sin() * size)
                    })
                    .collect();
                ctx.draw(&Points { coords: &points, color });
            }
        }

        // Draw selection indicator if selected
        if selected {
            let points: Vec<(f64, f64)> = (0..16)
                .map(|i| {
                    let angle = (i as f64) * std::f64::consts::PI / 8.0;
                    (x + angle.cos() * (size * 1.5), y + angle.sin() * (size * 1.5))
                })
                .collect();
            ctx.draw(&Points { coords: &points, color: Color::White });
        }

        // Draw node label
        let label_y = y + size * 2.0;
        let label = node.name.clone();
        let label_x = x - (label.len() as f64 * 0.4); // Adjusted multiplier to better center labels
        ctx.print(label_x, label_y, label);
    }

    pub fn get_selected_node(&self) -> Option<&NetworkNode> {
        self.selected_node.and_then(|id| self.nodes.get(&id))
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.2).min(5.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom * 0.8).max(0.2);
    }
}