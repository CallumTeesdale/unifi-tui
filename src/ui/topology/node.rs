use ratatui::style::Color;
use unifi_rs::device::DeviceState;
use uuid::Uuid;

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

impl NetworkNode {
    pub fn get_style(&self) -> (&'static str, Color) {
        match &self.node_type {
            NodeType::Device { device_type, state } => {
                let color = match state {
                    DeviceState::Online => Color::Green,
                    DeviceState::Offline => Color::Red,
                    _ => Color::Yellow,
                };

                match device_type {
                    DeviceType::AccessPoint => ("ap", color),
                    DeviceType::Switch => ("switch", color),
                    DeviceType::Gateway => ("gateway", color),
                    DeviceType::Other => ("device", color),
                }
            }
            NodeType::Client { client_type } => match client_type {
                ClientType::Wireless => ("wireless", Color::Yellow),
                ClientType::Wired => ("wired", Color::Blue),
                ClientType::Vpn => ("vpn", Color::Cyan),
            },
        }
    }
}
