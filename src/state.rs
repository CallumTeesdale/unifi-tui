use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tracing::instrument;
use unifi_rs::common::Page;
use unifi_rs::device::{DeviceDetails, DeviceOverview};
use unifi_rs::models::client::ClientOverview;
use unifi_rs::site::SiteOverview;
use unifi_rs::statistics::DeviceStatistics;
use unifi_rs::UnifiClient;
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
    pub tx_rate: i64,
    pub rx_rate: i64,
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
    pub sites: Vec<SiteOverview>,
    pub selected_site: Option<SiteContext>,
    pub devices: Vec<DeviceOverview>,
    pub clients: Vec<ClientOverview>,
    pub filtered_devices: Vec<DeviceOverview>,
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
    #[instrument(skip(client))]
    pub async fn new(client: UnifiClient) -> Result<Self> {
        tracing::info!("Initializing new AppState");
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

    pub async fn refresh_data(&mut self) -> Result<()> {
        if self.last_update.elapsed() < self.refresh_interval {
            return Ok(());
        }

        tracing::debug!("Starting data refresh");

        if let Err(e) = self.fetch_sites_and_data().await {
            tracing::error!(error = %e, "Failed to refresh data");
            self.set_error(format!("Error refreshing data: {}", e));
            return Err(e);
        }

        self.update_stats();
        self.apply_filters();
        self.last_update = Instant::now();
        Ok(())
    }

    #[instrument(skip(self), fields(site_id = ?self.selected_site.as_ref().map(|s| s.site_id)))]
    async fn fetch_sites_and_data(&mut self) -> Result<()> {
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
                tracing::debug!(site_id = ?site.site_id, "Fetching site data");
                self.fetch_site_data(site.site_id).await?;
            }
            None => {
                self.fetch_all_sites_data().await?;
            }
        }

        Ok(())
    }

    async fn fetch_site_data(&mut self, site_id: Uuid) -> Result<()> {
        let (devices, clients) = tokio::join!(
            self.fetch_all_paged_data(
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
            ),
            self.fetch_all_paged_data(
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
        );

        let (devices, clients) = (devices?, clients?);

        let mut device_data_futures = Vec::new();
        for device in &devices {
            let client = self.client.clone();
            let device_id = device.id;
            device_data_futures.push(async move {
                let details = client.get_device_details(site_id, device_id).await;
                let stats = client.get_device_statistics(site_id, device_id).await;
                (device_id, details, stats)
            });
        }
        
        for fut in device_data_futures {
            let (device_id, details, stats) = fut.await;
            if let Ok(details) = details {
                self.device_details.insert(device_id, details);
            }
            if let Ok(stats) = stats {
                self.device_stats.insert(device_id, stats.clone());
                self.update_network_history(device_id, &stats);
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

    #[instrument(skip(self, fetch_page))]
    async fn fetch_all_paged_data<T>(
        &self,
        fetch_page: impl Fn(i32, i32) -> Pin<Box<dyn Future<Output = Result<Page<T>>> + Send>> + Send,
        page_size: i32,
    ) -> Result<Vec<T>> {
        let mut all_items = Vec::new();
        let mut offset = 0;

        loop {
            tracing::debug!(offset, page_size, "Fetching page");
            let page = fetch_page(offset, page_size).await?;
            all_items.extend(page.data);

            if offset + page.count >= page.total_count {
                break;
            }
            offset += page_size;
        }

        tracing::debug!(items_count = all_items.len(), "Completed paged data fetch");
        Ok(all_items)
    }

    #[instrument(skip(self))]
    async fn fetch_all_sites_data(&mut self) -> Result<()> {
        self.devices.clear();
        self.clients.clear();
        self.device_details.clear();
        self.device_stats.clear();

        let site_ids: Vec<Uuid> = self.sites.iter().map(|s| s.id).collect();

        for site_id in site_ids {
            match self.fetch_site_data(site_id).await {
                Ok(_) => {
                    tracing::debug!(site_id = ?site_id, "Successfully fetched site data");
                }
                Err(e) => {
                    tracing::error!(
                        site_id = ?site_id,
                        error = %e,
                        "Failed to fetch site data"
                    );
                    self.set_error(format!("Error fetching data for site {}: {}", site_id, e));
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self, stats))]
    pub fn update_network_history(&mut self, device_id: Uuid, stats: &DeviceStatistics) {
        if let Some(uplink) = &stats.uplink {
            let history = self
                .network_history
                .entry(device_id)
                .or_insert_with(|| VecDeque::with_capacity(60));

            let throughput = NetworkThroughput {
                timestamp: Utc::now(),
                tx_rate: uplink.tx_rate_bps,
                rx_rate: uplink.rx_rate_bps,
            };

            if history.len() >= 60 {
                history.pop_front();
            }
            history.push_back(throughput);

            tracing::debug!(
                device_id = ?device_id,
                tx_rate = uplink.tx_rate_bps,
                rx_rate = uplink.rx_rate_bps,
                "Updated network history"
            );
        }
    }

    #[instrument(skip(self))]
    pub fn set_error(&mut self, message: String) {
        tracing::error!(error = %message);
        self.error_message = Some(message);
        self.error_timestamp = Some(Instant::now());
    }

    #[instrument(skip(self))]
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

        tracing::debug!(
            client_count = self.clients.len(),
            wireless_count = self
                .stats_history
                .back()
                .map(|s| s.wireless_clients)
                .unwrap_or(0),
            wired_count = self
                .stats_history
                .back()
                .map(|s| s.wired_clients)
                .unwrap_or(0),
            "Updated network stats"
        );
    }

    #[instrument(skip(self))]
    fn collect_device_metrics(&self) -> Vec<DeviceMetrics> {
        let metrics: Vec<DeviceMetrics> = self
            .devices
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
            .collect();

        tracing::debug!(metric_count = metrics.len(), "Collected device metrics");
        metrics
    }

    #[instrument(skip(self))]
    pub fn apply_filters(&mut self) {
        self.filtered_devices = self.devices.clone();
        self.filtered_clients = self.clients.clone();

        tracing::debug!(
            device_count = self.filtered_devices.len(),
            client_count = self.filtered_clients.len(),
            "Applied filters"
        );
    }

    #[instrument(skip(self))]
    pub fn set_site_context(&mut self, site_id: Option<Uuid>) {
        let previous_site = self.selected_site.as_ref().map(|s| s.site_id);

        self.selected_site = site_id.and_then(|id| {
            self.sites
                .iter()
                .find(|s| s.id == id)
                .map(|site| SiteContext {
                    site_id: id,
                    site_name: site.name.clone().unwrap_or_else(|| "Unnamed".to_string()),
                })
        });
        
        if previous_site != site_id {
            if let Some(site) = &self.selected_site {
                tracing::debug!(
                    site_id = ?site.site_id,
                    site_name = %site.site_name,
                    "Site context changed"
                );
            } else {
                tracing::debug!("Site context cleared");
            }
        }

        self.devices.clear();
        self.clients.clear();
        self.device_details.clear();
        self.device_stats.clear();
        self.last_update = Instant::now() - self.refresh_interval;
    }

    #[instrument(skip(self), fields(query_len = query.len()))]
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
                [
                    &d.name,
                    &d.model,
                    &d.mac_address,
                    &d.ip_address,
                    &format!("{:?}", d.state),
                ]
                .iter()
                .any(|field| field.to_lowercase().contains(&query))
            })
            .cloned()
            .collect();

        self.filtered_clients = self
            .clients
            .iter()
            .filter(|c| match c {
                ClientOverview::Wired(wc) => [
                    wc.base.name.as_deref().unwrap_or(""),
                    wc.base.ip_address.as_deref().unwrap_or(""),
                    &wc.mac_address,
                    &wc.uplink_device_id.to_string(),
                ]
                .iter()
                .any(|field| field.to_lowercase().contains(&query)),
                ClientOverview::Wireless(wc) => [
                    wc.base.name.as_deref().unwrap_or(""),
                    wc.base.ip_address.as_deref().unwrap_or(""),
                    &wc.mac_address,
                    &wc.uplink_device_id.to_string(),
                ]
                .iter()
                .any(|field| field.to_lowercase().contains(&query)),
                _ => false,
            })
            .cloned()
            .collect();

        tracing::trace!(
            query = %query,
            matches = self.filtered_devices.len() + self.filtered_clients.len(),
            "Search executed"
        );
    }
}
