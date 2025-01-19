use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use unifi_rs::{ClientOverview, DeviceDetails, DeviceStatistics, Page, UnifiClient};
use uuid::Uuid;

#[derive(Clone)]
pub struct SiteContext {
    pub site_id: Uuid,
    pub site_name: String,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct NetworkStats {
    pub timestamp: DateTime<Utc>,
    pub site_id: Option<Uuid>,
    pub client_count: usize,
    pub wireless_clients: usize,
    pub wired_clients: usize,
    pub device_stats: Vec<DeviceMetrics>,
}

#[allow(dead_code)]
pub struct NetworkThroughput {
    pub timestamp: DateTime<Utc>,
    pub tx_rate: f64,
    pub rx_rate: f64,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct DeviceMetrics {
    pub device_id: Uuid,
    pub device_name: String,
    pub cpu_utilization: Option<f64>,
    pub memory_utilization: Option<f64>,
    pub uptime: i64,
    pub tx_rate: Option<i64>,
    pub rx_rate: Option<i64>,
}

pub struct AppState {
    pub client: UnifiClient,
    pub sites: Vec<unifi_rs::SiteOverview>,
    pub selected_site: Option<SiteContext>,
    pub devices: Vec<unifi_rs::DeviceOverview>,
    pub clients: Vec<ClientOverview>,
    pub filtered_devices: Vec<unifi_rs::DeviceOverview>,
    pub filtered_clients: Vec<ClientOverview>,
    pub device_details: HashMap<Uuid, DeviceDetails>,
    pub device_stats: HashMap<Uuid, DeviceStatistics>,
    pub stats_history: VecDeque<NetworkStats>,
    pub last_update: Instant,
    pub refresh_interval: Duration,
    pub error_message: Option<String>,
    pub error_timestamp: Option<Instant>,
    pub network_history: HashMap<Uuid, VecDeque<NetworkThroughput>>,
}

impl AppState {
    pub async fn new(client: UnifiClient) -> Result<Self> {
        Ok(Self {
            client,
            sites: Vec::new(),
            selected_site: None,
            devices: Vec::new(),
            clients: Vec::new(),
            filtered_devices: Vec::new(),
            filtered_clients: Vec::new(),
            device_details: HashMap::new(),
            device_stats: HashMap::new(),
            stats_history: VecDeque::with_capacity(100),
            last_update: Instant::now(),
            refresh_interval: Duration::from_secs(5),
            error_message: None,
            error_timestamp: None,
            network_history: HashMap::new(),
        })
    }

    pub fn update_network_history(&mut self, device_id: Uuid, stats: &DeviceStatistics) {
        if let Some(uplink) = &stats.uplink {
            let history = self
                .network_history
                .entry(device_id)
                .or_insert_with(|| VecDeque::with_capacity(60));

            let throughput = NetworkThroughput {
                timestamp: Utc::now(),
                tx_rate: uplink.tx_rate_bps as f64 / 1_000_000.0,
                rx_rate: uplink.rx_rate_bps as f64 / 1_000_000.0,
            };

            if history.len() >= 60 {
                history.pop_front();
            }
            history.push_back(throughput);
        }
    }

    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.error_timestamp = Some(Instant::now());
    }

    async fn fetch_all_paged_data<T>(
        &self,
        fetch_page: impl Fn(i32, i32) -> Pin<Box<dyn Future<Output = Result<Page<T>>> + Send>> + Send,
        page_size: i32,
    ) -> Result<Vec<T>> {
        let mut all_items = Vec::new();
        let mut offset = 0;

        loop {
            let page = fetch_page(offset, page_size).await?;
            all_items.extend(page.data);

            if offset + page.count >= page.total_count {
                break;
            }
            offset += page_size;
        }

        Ok(all_items)
    }

    async fn fetch_site_data(&mut self, site_id: Uuid) -> Result<()> {
        let devices = self
            .fetch_all_paged_data(
                |offset, limit| {
                    let client = self.client.clone();
                    Box::pin(async move {
                        client
                            .list_devices(site_id, Some(offset), Some(limit))
                            .await
                            .map_err(AppError::UniFi)
                    })
                },
                25,
            )
            .await?;

        let clients = self
            .fetch_all_paged_data(
                |offset, limit| {
                    let client = self.client.clone();
                    Box::pin(async move {
                        client
                            .list_clients(site_id, Some(offset), Some(limit))
                            .await
                            .map_err(AppError::UniFi)
                    })
                },
                25,
            )
            .await?;

        for device in &devices {
            if let Ok(details) = self.client.get_device_details(site_id, device.id).await {
                self.device_details.insert(device.id, details);
            }
            if let Ok(stats) = self.client.get_device_statistics(site_id, device.id).await {
                self.device_stats.insert(device.id, stats.clone());
                self.update_network_history(device.id, &stats);
            }
        }

        if self.selected_site.as_ref().map(|s| s.site_id) == Some(site_id) {
            self.devices = devices;
            self.clients = clients;
        } else {
            self.devices.extend(devices);
            self.clients.extend(clients);
        }

        Ok(())
    }

    pub async fn refresh_data(&mut self) -> Result<()> {
        if self.last_update.elapsed() < self.refresh_interval {
            return Ok(());
        }

        let sites = self
            .fetch_all_paged_data(
                |offset, limit| {
                    let client = self.client.clone();
                    Box::pin(async move {
                        client
                            .list_sites(Some(offset), Some(limit))
                            .await
                            .map_err(AppError::UniFi)
                    })
                },
                25,
            )
            .await?;

        self.sites = sites;

        match &self.selected_site {
            Some(site) => {
                self.fetch_site_data(site.site_id).await?;
            }
            None => {
                self.devices.clear();
                self.clients.clear();
                self.device_details.clear();
                self.device_stats.clear();

                let site_ids: Vec<Uuid> = self.sites.iter().map(|s| s.id).collect();
                for site_id in site_ids {
                    if let Err(e) = self.fetch_site_data(site_id).await {
                        self.set_error(format!("Error fetching data for site {}: {}", site_id, e));
                    }
                }
            }
        }

        self.update_stats();
        self.apply_filters();
        self.last_update = Instant::now();
        Ok(())
    }

    fn update_stats(&mut self) {
        let stats = NetworkStats {
            timestamp: Utc::now(),
            site_id: self.selected_site.as_ref().map(|s| s.site_id),
            client_count: self.clients.len(),
            wireless_clients: self
                .clients
                .iter()
                .filter(|c| matches!(c, ClientOverview::Wireless(_)))
                .count(),
            wired_clients: self
                .clients
                .iter()
                .filter(|c| matches!(c, ClientOverview::Wired(_)))
                .count(),
            device_stats: self.collect_device_metrics(),
        };

        if self.stats_history.len() >= 100 {
            self.stats_history.pop_front();
        }
        self.stats_history.push_back(stats);
    }

    fn collect_device_metrics(&self) -> Vec<DeviceMetrics> {
        self.devices
            .iter()
            .filter_map(|device| {
                let stats = self.device_stats.get(&device.id)?;
                Some(DeviceMetrics {
                    device_id: device.id,
                    device_name: device.name.clone(),
                    cpu_utilization: stats.cpu_utilization_pct,
                    memory_utilization: stats.memory_utilization_pct,
                    uptime: stats.uptime_sec,
                    tx_rate: stats.uplink.as_ref().map(|u| u.tx_rate_bps),
                    rx_rate: stats.uplink.as_ref().map(|u| u.rx_rate_bps),
                })
            })
            .collect()
    }

    pub fn apply_filters(&mut self) {
        self.filtered_devices = self.devices.clone();
        self.filtered_clients = self.clients.clone();
    }

    pub fn set_site_context(&mut self, site_id: Option<Uuid>) {
        self.selected_site = site_id.and_then(|id| {
            self.sites
                .iter()
                .find(|s| s.id == id)
                .map(|site| SiteContext {
                    site_id: id,
                    site_name: site.name.clone().unwrap_or_else(|| "Unnamed".to_string()),
                })
        });

        self.devices.clear();
        self.clients.clear();
        self.device_details.clear();
        self.device_stats.clear();
        self.last_update = Instant::now() - self.refresh_interval;
    }

    pub fn search(&mut self, query: &str) {
        let query = query.to_lowercase();

        if query.is_empty() {
            self.filtered_devices = self.devices.clone();
            self.filtered_clients = self.clients.clone();
            return;
        }
        self.filtered_devices = self
            .devices
            .iter()
            .filter(|d| {
                d.name.to_lowercase().contains(&query)
                    || d.model.to_lowercase().contains(&query)
                    || d.mac_address.to_lowercase().contains(&query)
                    || d.ip_address.to_lowercase().contains(&query)
                    || format!("{:?}", d.state).to_lowercase().contains(&query)
            })
            .cloned()
            .collect();

        self.filtered_clients = self
            .clients
            .iter()
            .filter(|c| match c {
                ClientOverview::Wired(wc) => {
                    wc.base
                        .name
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
                        || wc
                            .base
                            .ip_address
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                        || wc.mac_address.to_lowercase().contains(&query)
                        || wc.uplink_device_id.to_string().contains(&query)
                }
                ClientOverview::Wireless(wc) => {
                    wc.base
                        .name
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
                        || wc
                            .base
                            .ip_address
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                        || wc.mac_address.to_lowercase().contains(&query)
                        || wc.uplink_device_id.to_string().contains(&query)
                }
                _ => false,
            })
            .cloned()
            .collect();
    }
}
