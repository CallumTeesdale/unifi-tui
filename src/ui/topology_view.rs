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
        match event.kind {
            MouseEventKind::Down(_) => {
                let canvas_x = ((event.column.saturating_sub(area.x + 1)) as f64 * 100.0)
                    / (area.width.saturating_sub(2) as f64);
                let canvas_y = ((event.row.saturating_sub(area.y + 1)) as f64 * 100.0)
                    / (area.height.saturating_sub(2) as f64);

                log::debug!("Mouse down at canvas coordinates: ({:.2}, {:.2})", canvas_x, canvas_y);

                self.selected_node = self.find_closest_node(canvas_x, canvas_y);
                self.dragging_node = self.selected_node;
                self.last_mouse_pos = (event.column, event.row);

                if let Some(id) = self.selected_node {
                    if let Some(node) = self.nodes.get(&id) {
                        log::debug!("Selected node: {}", node.name);
                    }
                }
            }
            MouseEventKind::Up(_) => {
                self.dragging_node = None;
            }
            MouseEventKind::Drag(_) => {
                let dx = (event.column as i32 - self.last_mouse_pos.0 as i32) as f64;
                let dy = (event.row as i32 - self.last_mouse_pos.1 as i32) as f64;

                // Scale movement by zoom level
                let world_dx = dx * self.canvas_dimensions.0 / (area.width as f64 * self.zoom);
                let world_dy = dy * self.canvas_dimensions.1 / (area.height as f64 * self.zoom);

                if let Some(id) = self.dragging_node {
                    if let Some(node) = self.nodes.get_mut(&id) {
                        node.x = (node.x + world_dx).clamp(0.0, self.canvas_dimensions.0);
                        node.y = (node.y + world_dy).clamp(0.0, self.canvas_dimensions.1);  // Removed the negative
                    }
                } else {
                    self.pan_offset.0 -= world_dx;
                    self.pan_offset.1 -= world_dy;  // Removed the negative
                }
                self.last_mouse_pos = (event.column, event.row);
            }
            _ => {}
        }
    }

    fn find_closest_node(&self, screen_x: f64, screen_y: f64) -> Option<Uuid> {
        const X_HIT_RADIUS: f64 = 8.0;
        const Y_SECTIONS: f64 = 4.0; // Divide the view into vertical sections

        // Get section of the click
        let click_section = (screen_y / (100.0 / Y_SECTIONS)).floor();

        // Group nodes by their vertical sections
        let mut nodes_by_section: Vec<(&Uuid, &NetworkNode, u32)> = self.nodes.iter()
            .map(|(id, node)| {
                let node_y = (node.y - self.pan_offset.1) * self.zoom;
                let section = (node_y / (100.0 / Y_SECTIONS)).floor();
                (id, node, section as u32)
            })
            .collect();

        // Sort nodes by section (with preference to upper sections)
        nodes_by_section.sort_by_key(|(_, _, section)| *section);

        // Find nodes in clicked section or the closest upper section
        for section in 0..=click_section as u32 {
            let section_nodes: Vec<_> = nodes_by_section.iter()
                .filter(|(_, _, s)| *s == section)
                .collect();

            // For nodes in this section, find the closest one horizontally
            let closest = section_nodes.iter()
                .min_by(|a, b| {
                    let a_x = (a.1.x - self.pan_offset.0) * self.zoom;
                    let b_x = (b.1.x - self.pan_offset.0) * self.zoom;
                    let a_dist = (a_x - screen_x).abs();
                    let b_dist = (b_x - screen_x).abs();
                    a_dist.partial_cmp(&b_dist).unwrap_or(std::cmp::Ordering::Equal)
                })
                .filter(|(_, node, _)| {
                    let node_x = (node.x - self.pan_offset.0) * self.zoom;
                    (node_x - screen_x).abs() < X_HIT_RADIUS
                });

            if let Some((&id, _, _)) = closest {
                let node = self.nodes.get(&id).unwrap();
                log::debug!(
                "Selected node in section {}: {} at ({:.2}, {:.2})",
                section,
                node.name,
                node.x,
                node.y
            );
                return Some(id);
            }
        }

        None
    }

    pub fn render(&self, ctx: &mut Context) {
        // Draw connections first
        for node in self.nodes.values() {
            if let Some(parent_id) = node.parent_id {
                if let Some(parent) = self.nodes.get(&parent_id) {
                    let (x1, y1) = ((node.x - self.pan_offset.0) * self.zoom,
                                    (node.y - self.pan_offset.1) * self.zoom);
                    let (x2, y2) = ((parent.x - self.pan_offset.0) * self.zoom,
                                    (parent.y - self.pan_offset.1) * self.zoom);

                    let color = match node.node_type {
                        NodeType::Client { client_type: ClientType::Wireless } => Color::Yellow,
                        NodeType::Client { client_type: ClientType::Wired } => Color::Blue,
                        _ => Color::Gray,
                    };

                    ctx.draw(&Line { x1, y1, x2, y2, color });
                }
            }
        }

        // Draw nodes
        for (id, node) in &self.nodes {
            let x = (node.x - self.pan_offset.0) * self.zoom;
            let y = (node.y - self.pan_offset.1) * self.zoom;
            let selected = Some(*id) == self.selected_node;

            let (shape, color) = self.get_node_style(node);
            self.draw_node(ctx, node, shape, color, selected);
        }
    }

    fn get_node_style(&self, node: &NetworkNode) -> (&'static str, Color) {
        match &node.node_type {
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
