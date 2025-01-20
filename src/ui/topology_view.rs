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
    pan_offset: (f64, f64),
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
            pan_offset: (0.0, 0.0), // Add this
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

            let parent_id = device_details
                .get(&device.id)
                .and_then(|d| d.uplink.as_ref().map(|u| u.device_id));

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
        // Find root nodes
        let root_nodes: Vec<Uuid> = self.nodes.values()
            .filter(|n| n.parent_id.is_none() || !self.nodes.contains_key(&n.parent_id.unwrap()))
            .map(|n| n.id)
            .collect();

        // Position root nodes
        let root_spacing = 100.0 / (root_nodes.len() + 1) as f64;
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
        let effective_area = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2)
        );

        // Convert mouse coordinates to relative position (0.0 to 1.0)
        let rel_x = (event.column.saturating_sub(effective_area.x) as f64) / (effective_area.width as f64);
        let rel_y = (event.row.saturating_sub(effective_area.y) as f64) / (effective_area.height as f64);

        // Scale to canvas coordinates
        let canvas_x = rel_x * 100.0;
        let canvas_y = rel_y * 100.0;

        log::debug!(
        "Mouse event - Screen: ({}, {}), Effective: ({}, {}), Canvas: ({:.2}, {:.2})", 
        event.column, event.row, 
        event.column.saturating_sub(effective_area.x),
        event.row.saturating_sub(effective_area.y),
        canvas_x, canvas_y
    );

        match event.kind {
            MouseEventKind::Down(_) => {
                log::debug!("Mouse down at canvas coordinates: ({:.2}, {:.2})", canvas_x, canvas_y);
                self.selected_node = self.find_node_at(canvas_x, canvas_y);
                self.dragging_node = self.selected_node;
                self.last_mouse_pos = (event.column, event.row);

                // Log selection result
                if let Some(id) = self.selected_node {
                    if let Some(node) = self.nodes.get(&id) {
                        log::debug!("Selected node: {}", node.name);
                    }
                } else {
                    log::debug!("No node selected");
                }
            }
            MouseEventKind::Up(_) => {
                self.dragging_node = None;
            }
            MouseEventKind::Drag(_) => {
                let dx = (event.column as i32 - self.last_mouse_pos.0 as i32) as f64
                    * (self.canvas_dimensions.0 / effective_area.width as f64);
                let dy = (event.row as i32 - self.last_mouse_pos.1 as i32) as f64
                    * (self.canvas_dimensions.1 / effective_area.height as f64);

                if let Some(id) = self.dragging_node {
                    if let Some(node) = self.nodes.get_mut(&id) {
                        node.x = (node.x + dx / self.zoom).clamp(0.0, self.canvas_dimensions.0);
                        node.y = (node.y + dy / self.zoom).clamp(0.0, self.canvas_dimensions.1);
                    }
                } else {
                    self.pan_offset.0 += dx / self.zoom;
                    self.pan_offset.1 += dy / self.zoom;
                }
                self.last_mouse_pos = (event.column, event.row);
            }
            _ => {}
        }
    }

    fn find_node_at(&self, screen_x: f64, screen_y: f64) -> Option<Uuid> {
        const HIT_RADIUS: f64 = 5.0;

        let nodes_with_distances: Vec<(&Uuid, &NetworkNode, f64)> = self.nodes.iter()
            .map(|(id, node)| {
                let dx = node.x - screen_x;
                let dy = node.y - screen_y;
                let distance = (dx * dx + dy * dy).sqrt();

                log::info!(
                "Node '{}' at ({:.2}, {:.2}), click at ({:.2}, {:.2}), distance: {:.2}",
                node.name, node.x, node.y, screen_x, screen_y, distance
            );

                (id, node, distance)
            })
            .collect();

        // Sort by distance
        let mut sorted_nodes = nodes_with_distances;
        sorted_nodes.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        // Find closest node within hit radius
        if let Some((id, node, distance)) = sorted_nodes.first() {
            if *distance < HIT_RADIUS {
                log::debug!(
                "Selected node: '{}' at ({:.2}, {:.2}), distance: {:.2}",
                node.name, node.x, node.y, distance
            );
                return Some(**id);
            }
        }

        None
    }

    pub fn render(&self, ctx: &mut Context) {
        // Add transform function for coordinates
        let transform_coord = |x: f64, y: f64| -> (f64, f64) {
            (
                (x - self.pan_offset.0) * self.zoom,
                (y - self.pan_offset.1) * self.zoom,
            )
        };

        // Draw connections first
        for node in self.nodes.values() {
            if let Some(parent_id) = node.parent_id {
                if let Some(parent) = self.nodes.get(&parent_id) {
                    let (x1, y1) = transform_coord(node.x, node.y);
                    let (x2, y2) = transform_coord(parent.x, parent.y);

                    let color = match node.node_type {
                        NodeType::Client {
                            client_type: ClientType::Wireless,
                        } => Color::Yellow,
                        NodeType::Client {
                            client_type: ClientType::Wired,
                        } => Color::Blue,
                        _ => Color::Gray,
                    };

                    ctx.draw(&Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        color,
                    });
                }
            }
        }

        // Draw nodes with transformed coordinates
        for (id, node) in &self.nodes {
            let (x, y) = transform_coord(node.x, node.y);
            let selected = Some(*id) == self.selected_node;

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

            self.draw_node(ctx, node, shape, color, selected);
        }
    }

    fn draw_node(
        &self,
        ctx: &mut Context,
        node: &NetworkNode,
        shape: &str,
        color: Color,
        selected: bool,
    ) {
        let x = (node.x - self.pan_offset.0) * self.zoom;
        let y = (node.y - self.pan_offset.1) * self.zoom;
        let base_size = if selected { 3.0 } else { 2.0 };
        let size = base_size * self.zoom; // Scale size with zoom

        match shape {
            "ap" => {
                for i in 0..3 {
                    let radius = size - (i as f64 * 0.5 * self.zoom);
                    let points: Vec<(f64, f64)> = (0..16)
                        .map(|j| {
                            let angle = (j as f64) * std::f64::consts::PI / 8.0;
                            (x + angle.cos() * radius, y + angle.sin() * radius)
                        })
                        .collect();
                    ctx.draw(&Points {
                        coords: &points,
                        color,
                    });
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
                ctx.draw(&Points {
                    coords: &points,
                    color,
                });
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
                ctx.draw(&Points {
                    coords: &points,
                    color,
                });
            }
        }

        // Draw selection indicator if selected
        if selected {
            // Draw a small dot in the center instead of the large highlight ring
            ctx.draw(&Points {
                coords: &[(x, y)],
                color: Color::White,
            });
        }

        // Draw node label
        let label_y = y + size * 2.0;
        let label = node.name.clone();
        let label_x = x - (label.len() as f64 * 0.4 * self.zoom);
        ctx.print(label_x, label_y, label);
    }

    pub fn get_selected_node(&self) -> Option<&NetworkNode> {
        self.selected_node.and_then(|id| self.nodes.get(&id))
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.2).min(5.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.2).max(0.2);
    }
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_offset = (0.0, 0.0);
        self.initialize_layout();

        // Calculate bounds to center the view
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for node in self.nodes.values() {
            min_x = min_x.min(node.x);
            min_y = min_y.min(node.y);
            max_x = max_x.max(node.x);
            max_y = max_y.max(node.y);
        }

        // Center the view
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        self.pan_offset = (center_x - 50.0, center_y - 50.0);
    }
}
