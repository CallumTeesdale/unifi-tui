use crate::ui::topology::node::{ClientType, DeviceType, NetworkNode, NodeType};
use crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::Color,
    widgets::canvas::{Context, Line, Points},
};
use std::collections::HashMap;
use unifi_rs::device::{DeviceDetails, DeviceOverview};
use unifi_rs::models::client::ClientOverview;
use uuid::Uuid;

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
            pan_offset: (0.0, 0.0),
            zoom: 1.0,
            canvas_dimensions: (100.0, 100.0),
        }
    }
}

/// State And Layout
impl TopologyView {
    pub fn update_from_state(
        &mut self,
        devices: &[DeviceOverview],
        clients: &[ClientOverview],
        device_details: &HashMap<Uuid, DeviceDetails>,
    ) {
        self.nodes.clear();

        // Add all device nodes to the network map
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

        // Add all our client nodes to the network map
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

        // Update the layout children for each node
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
        // Find  root nodes (nodes without a parent or with a parent that doesn't exist) like our gateway device
        let root_nodes: Vec<Uuid> = self
            .nodes
            .values()
            .filter(|n| n.parent_id.is_none() || !self.nodes.contains_key(&n.parent_id.unwrap()))
            .map(|n| n.id)
            .collect();

        // Place root nodes at the top of the canvas to mimic unifi tree layout
        let root_spacing = 100.0 / (root_nodes.len() + 1) as f64;
        for (i, id) in root_nodes.iter().enumerate() {
            if let Some(node) = self.nodes.get_mut(id) {
                node.x = root_spacing * (i + 1) as f64;
                node.y = 20.0;
            }
        }

        // iter through root nodes and layout their children
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
}

/// Mouse Interaction
impl TopologyView {
    pub fn handle_mouse_event(&mut self, event: MouseEvent, area: Rect) {
        match event.kind {
            MouseEventKind::Down(_) => {
                let canvas_x = (event.column.saturating_sub(area.x + 1) as f64 * 100.0)
                    / (area.width.saturating_sub(2) as f64);
                let canvas_y = (event.row.saturating_sub(area.y + 1) as f64 * 100.0)
                    / (area.height.saturating_sub(2) as f64);

                self.selected_node = self.find_closest_node(canvas_x, canvas_y);
                self.dragging_node = self.selected_node;
                self.last_mouse_pos = (event.column, event.row);
            }
            MouseEventKind::Up(_) => {
                self.dragging_node = None;
            }
            MouseEventKind::Drag(_) => {
                let dx = (event.column as i32 - self.last_mouse_pos.0 as i32) as f64;
                let dy = (event.row as i32 - self.last_mouse_pos.1 as i32) as f64;

                // ivert y to make it feel more natural and scale with zoom
                let world_dx = dx * self.canvas_dimensions.0 / (area.width as f64 * self.zoom);
                let world_dy = -dy * self.canvas_dimensions.1 / (area.height as f64 * self.zoom);

                if let Some(id) = self.dragging_node {
                    if let Some(node) = self.nodes.get_mut(&id) {
                        node.x = (node.x + world_dx).clamp(0.0, self.canvas_dimensions.0);
                        node.y = (node.y + world_dy).clamp(0.0, self.canvas_dimensions.1);
                    }
                } else {
                    self.pan_offset.0 -= world_dx;
                    self.pan_offset.1 -= world_dy;
                }
                self.last_mouse_pos = (event.column, event.row);
            }
            _ => {}
        }
    }

    fn find_closest_node(&self, click_x: f64, click_y: f64) -> Option<Uuid> {
        // Canvas uses normalized coordinates (0-100) with origin at top-left
        let click_y = 100.0 - click_y;

        // Calculate node positions with current zoom and pan offset since we may be zoomed in or panned
        let nodes_with_pos: Vec<_> = self
            .nodes
            .iter()
            .map(|(id, node)| {
                let x = (node.x - self.pan_offset.0) * self.zoom;
                let y = (node.y - self.pan_offset.1) * self.zoom;
                (id, node, x, y)
            })
            .collect();

        // if we ckick on a node, return the id by finding the closest node to the click
        nodes_with_pos
            .into_iter()
            .filter(|(_, _, x, y)| {
                let dx = x - click_x;
                let dy = y - click_y;
                let distance = (dx * dx + dy * dy).sqrt();
                distance < (8.0 * self.zoom) // Scale hit radius with zoom
            })
            .min_by(|(_, _, x1, y1), (_, _, x2, y2)| {
                let dist1 = ((x1 - click_x).powi(2) + (y1 - click_y).powi(2)).sqrt();
                let dist2 = ((x2 - click_x).powi(2) + (y2 - click_y).powi(2)).sqrt();
                dist1
                    .partial_cmp(&dist2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _node, _, _)| *id)
    }
}

/// Rendering
impl TopologyView {
    pub fn render(&self, ctx: &mut Context) {
        // We start by drawing the connections between nodes first since tree layout is top-down
        for node in self.nodes.values() {
            if let Some(parent_id) = node.parent_id {
                if let Some(parent) = self.nodes.get(&parent_id) {
                    let (x1, y1) = (
                        (node.x - self.pan_offset.0) * self.zoom,
                        (node.y - self.pan_offset.1) * self.zoom,
                    );
                    let (x2, y2) = (
                        (parent.x - self.pan_offset.0) * self.zoom,
                        (parent.y - self.pan_offset.1) * self.zoom,
                    );

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

        // Draw nodes on top of connections
        for (id, node) in &self.nodes {
            let selected = Some(*id) == self.selected_node;

            let (shape, color) = node.get_style();
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
                    let points = circle(x, y, radius);
                    ctx.draw(&Points {
                        coords: &points,
                        color,
                    });
                }
            }
            "switch" => {
                let points = [
                    (x - size, y - size / 2.0),
                    (x + size, y - size / 2.0),
                    (x + size, y + size / 2.0),
                    (x - size, y + size / 2.0),
                ];
                square(ctx, color, &points);
            }
            "wireless" => {
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
                let points = [
                    (x - size * 0.5, y - size * 0.5),
                    (x + size * 0.5, y - size * 0.5),
                    (x + size * 0.5, y + size * 0.5),
                    (x - size * 0.5, y + size * 0.5),
                ];
                square(ctx, color, &points);
            }
            _ => {
                let points = circle(x, y, size);
                ctx.draw(&Points {
                    coords: &points,
                    color,
                });
            }
        }

        // Selected we found a hit
        if selected {
            // Inidcate to the user that the node is selected
            ctx.draw(&Points {
                coords: &[(x, y)],
                color: Color::White,
            });
        }

        // Label for the node should be the name of the node
        let label_y = y + size * 2.0;
        let label = node.name.clone();
        let label_x = x - (label.len() as f64 * 0.4 * self.zoom);
        ctx.print(label_x, label_y, label);
    }
}

/// Viewport Control
impl TopologyView {
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

        // centre the view on the nodes
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        self.pan_offset = (center_x - 50.0, center_y - 50.0);
    }
}

fn circle(x: f64, y: f64, size: f64) -> Vec<(f64, f64)> {
    let points: Vec<(f64, f64)> = (0..16)
        .map(|i| {
            let angle = (i as f64) * std::f64::consts::PI / 8.0;
            (x + angle.cos() * size, y + angle.sin() * size)
        })
        .collect();
    points
}

fn square(ctx: &mut Context, color: Color, points: &[(f64, f64); 4]) {
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
