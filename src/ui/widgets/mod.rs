pub mod client_stats;
pub mod device_stats;

pub use device_stats::DeviceStatsView;

pub fn format_network_speed(bps: i64) -> String {
    if bps >= 1_000_000_000 {
        format!("{:.2} Gbps", bps as f64 / 1_000_000_000.0)
    } else if bps >= 1_000_000 {
        format!("{:.2} Mbps", bps as f64 / 1_000_000.0)
    } else if bps >= 1_000 {
        format!("{:.2} Kbps", bps as f64 / 1_000.0)
    } else {
        format!("{} bps", bps)
    }
}
