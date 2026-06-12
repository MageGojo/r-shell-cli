//! Live server resource monitoring, FinalShell-style.
//!
//! A single SSH command snapshots `/proc` and `df`; [`parse_snapshot`] turns the
//! raw text into a [`StatsSnapshot`]. Two consecutive snapshots are diffed by
//! [`SystemStats::from_samples`] to derive CPU% and network throughput, which are
//! rate quantities that need a time delta to compute.

/// Marker emitted between sections of the remote stats command so we can split
/// the combined output back into its parts reliably.
pub const SECTION_MARKER: &str = "===RSHELL-SECTION===";

/// One remote command that gathers everything we need in a single round-trip.
/// Each section is preceded by [`SECTION_MARKER`] so parsing stays robust even
/// when individual files are missing (non-Linux hosts simply yield empty parts).
pub fn stats_command() -> String {
    let m = SECTION_MARKER;
    format!(
        "echo {m}; cat /proc/stat 2>/dev/null; \
         echo {m}; cat /proc/meminfo 2>/dev/null; \
         echo {m}; cat /proc/loadavg 2>/dev/null; \
         echo {m}; cat /proc/uptime 2>/dev/null; \
         echo {m}; cat /proc/net/dev 2>/dev/null; \
         echo {m}; df -kP / 2>/dev/null; \
         echo {m}; nproc 2>/dev/null; \
         echo {m}; uname -sr 2>/dev/null"
    )
}

/// Cumulative CPU jiffies parsed from the aggregate `cpu` line of `/proc/stat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub total: u64,
}

/// A raw, point-in-time snapshot. Absolute counters here are converted to rates
/// once paired with a previous snapshot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatsSnapshot {
    pub cpu: Option<CpuTimes>,
    pub mem_total_kb: u64,
    pub mem_available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
    pub load1: f32,
    pub uptime_secs: f64,
    /// Cumulative bytes received / transmitted across all real interfaces.
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub disk_total_kb: u64,
    pub disk_used_kb: u64,
    pub cpu_cores: u32,
    pub os: String,
}

/// Derived, display-ready metrics. Percentages are 0.0..=100.0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub mem_percent: f32,
    pub mem_used_kb: u64,
    pub mem_total_kb: u64,
    pub swap_percent: f32,
    pub swap_used_kb: u64,
    pub swap_total_kb: u64,
    pub disk_percent: f32,
    pub disk_used_kb: u64,
    pub disk_total_kb: u64,
    /// Bytes per second.
    pub net_rx_per_sec: f64,
    pub net_tx_per_sec: f64,
    pub load1: f32,
    pub uptime_secs: f64,
    pub cpu_cores: u32,
    pub os: String,
}

fn section<'a>(parts: &'a [&'a str], index: usize) -> &'a str {
    parts.get(index).copied().unwrap_or("")
}

/// Parse the combined output of [`stats_command`] into a [`StatsSnapshot`].
pub fn parse_snapshot(raw: &str) -> StatsSnapshot {
    let parts: Vec<&str> = raw.split(SECTION_MARKER).skip(1).collect();
    let mut snap = StatsSnapshot::default();

    snap.cpu = parse_cpu(section(&parts, 0));

    let (mem_total, mem_avail, mem_free, buffers, cached, swap_total, swap_free) =
        parse_meminfo(section(&parts, 1));
    snap.mem_total_kb = mem_total;
    // Prefer MemAvailable; fall back to free + buffers + cached on old kernels.
    snap.mem_available_kb = if mem_avail > 0 {
        mem_avail
    } else {
        mem_free + buffers + cached
    };
    snap.swap_total_kb = swap_total;
    snap.swap_free_kb = swap_free;

    snap.load1 = parse_loadavg(section(&parts, 2));
    snap.uptime_secs = parse_uptime(section(&parts, 3));

    let (rx, tx) = parse_net_dev(section(&parts, 4));
    snap.net_rx_bytes = rx;
    snap.net_tx_bytes = tx;

    let (disk_total, disk_used) = parse_df(section(&parts, 5));
    snap.disk_total_kb = disk_total;
    snap.disk_used_kb = disk_used;

    snap.cpu_cores = section(&parts, 6).trim().parse().unwrap_or(0);
    snap.os = section(&parts, 7).trim().to_string();

    snap
}

fn parse_cpu(section: &str) -> Option<CpuTimes> {
    for line in section.lines() {
        if let Some(rest) = line.strip_prefix("cpu ") {
            let values: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|v| v.parse::<u64>().ok())
                .collect();
            if values.len() >= 4 {
                // user nice system idle iowait irq softirq steal ...
                let idle = values[3] + values.get(4).copied().unwrap_or(0);
                let total: u64 = values.iter().sum();
                return Some(CpuTimes { idle, total });
            }
        }
    }
    None
}

#[allow(clippy::type_complexity)]
fn parse_meminfo(section: &str) -> (u64, u64, u64, u64, u64, u64, u64) {
    let mut total = 0;
    let mut available = 0;
    let mut free = 0;
    let mut buffers = 0;
    let mut cached = 0;
    let mut swap_total = 0;
    let mut swap_free = 0;
    for line in section.lines() {
        let mut it = line.split_whitespace();
        let Some(key) = it.next() else { continue };
        let value: u64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        match key {
            "MemTotal:" => total = value,
            "MemAvailable:" => available = value,
            "MemFree:" => free = value,
            "Buffers:" => buffers = value,
            "Cached:" => cached = value,
            "SwapTotal:" => swap_total = value,
            "SwapFree:" => swap_free = value,
            _ => {}
        }
    }
    (
        total, available, free, buffers, cached, swap_total, swap_free,
    )
}

fn parse_loadavg(section: &str) -> f32 {
    section
        .split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}

fn parse_uptime(section: &str) -> f64 {
    section
        .split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}

/// Sum rx/tx bytes across all interfaces except loopback.
fn parse_net_dev(section: &str) -> (u64, u64) {
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in section.lines() {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name == "lo" || name.is_empty() {
            continue;
        }
        let cols: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|v| v.parse::<u64>().ok())
            .collect();
        // /proc/net/dev: rx_bytes is col 0, tx_bytes is col 8.
        if cols.len() >= 9 {
            rx = rx.saturating_add(cols[0]);
            tx = tx.saturating_add(cols[8]);
        }
    }
    (rx, tx)
}

/// Parse `df -kP /`: take the last data row's total + used (in KB).
fn parse_df(section: &str) -> (u64, u64) {
    for line in section.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Filesystem 1024-blocks Used Available Capacity Mounted-on
        if cols.len() >= 4 {
            let total = cols[1].parse::<u64>().unwrap_or(0);
            let used = cols[2].parse::<u64>().unwrap_or(0);
            if total > 0 {
                return (total, used);
            }
        }
    }
    (0, 0)
}

impl SystemStats {
    /// Combine two snapshots taken `elapsed_secs` apart into display metrics.
    /// The CPU and network figures are rates, so they need the previous sample;
    /// everything else is taken from `current`.
    pub fn from_samples(
        previous: Option<&StatsSnapshot>,
        current: &StatsSnapshot,
        elapsed_secs: f64,
    ) -> Self {
        let mut stats = SystemStats {
            mem_total_kb: current.mem_total_kb,
            swap_total_kb: current.swap_total_kb,
            disk_total_kb: current.disk_total_kb,
            disk_used_kb: current.disk_used_kb,
            load1: current.load1,
            uptime_secs: current.uptime_secs,
            cpu_cores: current.cpu_cores,
            os: current.os.clone(),
            ..Default::default()
        };

        if current.mem_total_kb > 0 {
            stats.mem_used_kb = current
                .mem_total_kb
                .saturating_sub(current.mem_available_kb);
            stats.mem_percent =
                (stats.mem_used_kb as f32 / current.mem_total_kb as f32 * 100.0).clamp(0.0, 100.0);
        }

        if current.swap_total_kb > 0 {
            stats.swap_used_kb = current.swap_total_kb.saturating_sub(current.swap_free_kb);
            stats.swap_percent = (stats.swap_used_kb as f32 / current.swap_total_kb as f32 * 100.0)
                .clamp(0.0, 100.0);
        }

        if current.disk_total_kb > 0 {
            stats.disk_percent = (current.disk_used_kb as f32 / current.disk_total_kb as f32
                * 100.0)
                .clamp(0.0, 100.0);
        }

        if let (Some(prev), Some(cur)) = (previous.and_then(|p| p.cpu), current.cpu) {
            let total_delta = cur.total.saturating_sub(prev.total);
            let idle_delta = cur.idle.saturating_sub(prev.idle);
            if total_delta > 0 {
                let busy = total_delta.saturating_sub(idle_delta) as f32;
                stats.cpu_percent = (busy / total_delta as f32 * 100.0).clamp(0.0, 100.0);
            }
        }

        if let Some(prev) = previous {
            if elapsed_secs > 0.05 {
                let rx_delta = current.net_rx_bytes.saturating_sub(prev.net_rx_bytes);
                let tx_delta = current.net_tx_bytes.saturating_sub(prev.net_tx_bytes);
                stats.net_rx_per_sec = rx_delta as f64 / elapsed_secs;
                stats.net_tx_per_sec = tx_delta as f64 / elapsed_secs;
            }
        }

        stats
    }
}

/// Human-readable byte rate, e.g. `1.5 MB/s`.
pub fn format_rate(bytes_per_sec: f64) -> String {
    format!("{}/s", format_bytes_f64(bytes_per_sec))
}

/// Human-readable byte size from a float.
pub fn format_bytes_f64(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes.max(0.0);
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{:.0} {}", value, UNITS[unit])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

/// Compact `used/total` sharing one unit (scaled to the total), e.g. `4.9/7.8 GB`.
pub fn format_kb_pair(used_kb: u64, total_kb: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut total = total_kb as f64 * 1024.0;
    let mut used = used_kb as f64 * 1024.0;
    let mut unit = 0;
    while total >= 1024.0 && unit < UNITS.len() - 1 {
        total /= 1024.0;
        used /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{used:.0}/{total:.0} {}", UNITS[unit])
    } else {
        format!("{used:.1}/{total:.1} {}", UNITS[unit])
    }
}

/// Format an uptime in seconds as `Xd Yh Zm`.
pub fn format_uptime(secs: f64) -> String {
    let total = secs as u64;
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let minutes = (total % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_snapshot() {
        let raw = format!(
            "{m}\ncpu  100 0 50 800 50 0 0 0\ncpu0 50 0 25 400 25 0 0 0\n\
             {m}\nMemTotal:       2048000 kB\nMemAvailable:    1024000 kB\nSwapTotal: 1000 kB\nSwapFree: 400 kB\n\
             {m}\n0.42 0.30 0.25 1/200 1234\n\
             {m}\n3600.50 7000.00\n\
             {m}\nInter-|   Receive\n face |bytes    packets\n  lo:  100 1 0 0 0 0 0 0  200 1 0 0 0 0 0 0\n eth0: 1000 5 0 0 0 0 0 0 2000 5 0 0 0 0 0 0\n\
             {m}\nFilesystem 1024-blocks Used Available Capacity Mounted\n/dev/sda1 10000000 4000000 6000000 40% /\n\
             {m}\n4\n\
             {m}\nLinux 6.1.0\n",
            m = SECTION_MARKER
        );
        let snap = parse_snapshot(&raw);
        assert_eq!(snap.mem_total_kb, 2_048_000);
        assert_eq!(snap.mem_available_kb, 1_024_000);
        assert_eq!(snap.swap_total_kb, 1000);
        assert_eq!(snap.swap_free_kb, 400);
        assert!((snap.load1 - 0.42).abs() < 1e-6);
        assert!((snap.uptime_secs - 3600.50).abs() < 1e-6);
        // eth0 only (lo excluded): rx=1000, tx=2000.
        assert_eq!(snap.net_rx_bytes, 1000);
        assert_eq!(snap.net_tx_bytes, 2000);
        assert_eq!(snap.disk_total_kb, 10_000_000);
        assert_eq!(snap.disk_used_kb, 4_000_000);
        assert_eq!(snap.cpu_cores, 4);
        assert_eq!(snap.os, "Linux 6.1.0");
        let cpu = snap.cpu.unwrap();
        assert_eq!(cpu.idle, 850); // idle 800 + iowait 50
        assert_eq!(cpu.total, 1000);
    }

    #[test]
    fn derives_cpu_and_network_rates() {
        let prev = StatsSnapshot {
            cpu: Some(CpuTimes {
                idle: 800,
                total: 1000,
            }),
            net_rx_bytes: 1000,
            net_tx_bytes: 2000,
            ..Default::default()
        };
        let cur = StatsSnapshot {
            cpu: Some(CpuTimes {
                idle: 850,
                total: 1100,
            }),
            mem_total_kb: 2_000_000,
            mem_available_kb: 1_000_000,
            net_rx_bytes: 3000,
            net_tx_bytes: 4000,
            ..Default::default()
        };
        let stats = SystemStats::from_samples(Some(&prev), &cur, 2.0);
        // busy=100-50=50 over total delta 100 => 50%.
        assert!((stats.cpu_percent - 50.0).abs() < 1e-3);
        assert!((stats.mem_percent - 50.0).abs() < 1e-3);
        // (3000-1000)/2s = 1000 B/s.
        assert!((stats.net_rx_per_sec - 1000.0).abs() < 1e-6);
        assert!((stats.net_tx_per_sec - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn formats_helpers() {
        assert_eq!(format_bytes_f64(512.0), "512 B");
        assert_eq!(format_rate(1536.0), "1.5 KB/s");
        assert_eq!(format_uptime(90_061.0), "1d 1h 1m");
        assert_eq!(format_kb_pair(5_100_000, 8_192_000), "4.9/7.8 GB");
        assert_eq!(format_kb_pair(512, 1000), "512.0/1000.0 KB");
    }
}
