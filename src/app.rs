use crate::state::AppState;
use crate::ui::widgets::DeviceStatsView;
use ratatui::widgets::TableState;
use unifi_rs::models::client::ClientOverview;
use uuid::Uuid;
use crate::ui::topology_view::TopologyView;

#[derive(PartialEq, Clone)]
pub enum Mode {
    Overview,
    DeviceDetail,
    ClientDetail,
    #[allow(dead_code)]
    Help,
}

#[derive(PartialEq, Clone)]
pub enum DialogType {
    Confirmation,
    #[allow(dead_code)] // Not used yet
    Message,
    #[allow(dead_code)] // Not used yet
    Error,
}

#[derive(Clone, Copy)]
pub enum SortOrder {
    Ascending,
    Descending,
    None,
}

pub type Callback = Box<dyn FnOnce(&mut App) -> anyhow::Result<()> + Send>;

pub struct Dialog {
    pub title: String,
    pub message: String,
    pub dialog_type: DialogType,
    pub callback: Option<Callback>,
}

pub struct App {
    pub state: AppState,
    pub current_tab: usize,
    pub mode: Mode,
    pub dialog: Option<Dialog>,
    pub search_mode: bool,
    pub search_query: String,
    pub show_help: bool,
    pub device_sort_column: usize,
    pub device_sort_order: SortOrder,
    pub client_sort_column: usize,
    pub client_sort_order: SortOrder,
    pub sites_table_state: TableState,
    pub devices_table_state: TableState,
    pub device_stats_view: Option<DeviceStatsView>,
    pub clients_table_state: TableState,
    pub selected_device_id: Option<Uuid>,
    pub selected_client_id: Option<Uuid>,
    pub topology_view: TopologyView,
    pub should_quit: bool,
}

impl App {
    pub async fn new(state: AppState) -> anyhow::Result<Self> {
        Ok(Self {
            state,
            current_tab: 0,
            mode: Mode::Overview,
            dialog: None,
            search_mode: false,
            search_query: String::new(),
            show_help: false,
            device_sort_column: 0,
            device_sort_order: SortOrder::None,
            client_sort_column: 0,
            client_sort_order: SortOrder::None,
            sites_table_state: TableState::default(),
            devices_table_state: TableState::default(),
            clients_table_state: TableState::default(),
            selected_device_id: None,
            selected_client_id: None,
            device_stats_view: None,
            topology_view: TopologyView::new(),
            should_quit: false,
        })
    }

    pub async fn refresh(&mut self) -> anyhow::Result<()> {
        self.state.refresh_data().await?;
        
        self.topology_view.update_from_state(
            &self.state.filtered_devices,
            &self.state.filtered_clients,
            &self.state.device_details,
        );

        Ok(())
    }
    pub fn sort_devices(&mut self) {
        if matches!(self.device_sort_order, SortOrder::None) {
            return;
        }

        self.state.filtered_devices.sort_by(|a, b| {
            let cmp = match self.device_sort_column {
                0 => a.name.cmp(&b.name),
                1 => a.model.cmp(&b.model),
                2 => a.mac_address.cmp(&b.mac_address),
                3 => a.ip_address.cmp(&b.ip_address),
                4 => format!("{:?}", a.state).cmp(&format!("{:?}", b.state)),
                _ => std::cmp::Ordering::Equal,
            };
            match self.device_sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
                SortOrder::None => cmp,
            }
        });
    }

    pub fn sort_clients(&mut self) {
        if matches!(self.client_sort_order, SortOrder::None) {
            return;
        }

        self.state.filtered_clients.sort_by(|a, b| {
            let (a_name, a_ip, a_mac) = match a {
                ClientOverview::Wired(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                ClientOverview::Wireless(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                _ => ("", "", ""),
            };

            let (b_name, b_ip, b_mac) = match b {
                ClientOverview::Wired(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                ClientOverview::Wireless(c) => (
                    c.base.name.as_deref().unwrap_or(""),
                    c.base.ip_address.as_deref().unwrap_or(""),
                    c.mac_address.as_str(),
                ),
                _ => ("", "", ""),
            };

            let cmp = match self.client_sort_column {
                0 => a_name.cmp(b_name),
                1 => a_ip.cmp(b_ip),
                2 => a_mac.cmp(b_mac),
                _ => std::cmp::Ordering::Equal,
            };
            match self.client_sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
                SortOrder::None => cmp,
            }
        });
    }

    pub fn next_tab(&mut self) {
        self.current_tab = (self.current_tab + 1) % 5;
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = (self.current_tab + 3) % 5;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        if self.show_help {
            self.search_mode = false;
        }
    }
    pub fn enter_search_mode(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
    }

    pub fn exit_search_mode(&mut self) {
        self.search_mode = false;
    }

    pub fn clear_search(&mut self) {
        self.search_mode = false;
        self.search_query.clear();
        self.state.apply_filters();
    }

    pub fn select_device(&mut self, device_id: Option<Uuid>) {
        self.selected_device_id = device_id;
        if let Some(id) = device_id {
            self.mode = Mode::DeviceDetail;
            self.device_stats_view = Some(DeviceStatsView::new(id, 0));
        } else {
            self.device_stats_view = None;
        }
    }

    pub fn select_client(&mut self, client_id: Option<Uuid>) {
        self.selected_client_id = client_id;
        if client_id.is_some() {
            self.mode = Mode::ClientDetail;
        }
    }

    pub fn back_to_overview(&mut self) {
        self.mode = Mode::Overview;
        self.selected_device_id = None;
        self.selected_client_id = None;
    }
}
