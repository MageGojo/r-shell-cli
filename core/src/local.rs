//! 本机(运行 GUI 的这台电脑)系统指标采集 —— 跨平台,基于 `sysinfo`。
//!
//! 远端监控走 SSH/ADB 跑 `/proc`+`df`(见 [`crate::monitor`]),但**本机**在 macOS 上
//! 没有 `/proc`,故本机一侧用 `sysinfo`(Linux / macOS / Windows 通吃)采集,产出与
//! 远端**同一种** [`StatsSnapshot`],从而复用 [`SystemStats::from_samples`] 的差分
//! (CPU% / 网速需两帧)与同一套 DTO / UI。
//!
//! [`LocalCollector`] 持有 `sysinfo` 句柄**跨采样复用**(CPU% 需要两次刷新间的增量),
//! 因此调用方应保留同一个实例按周期 [`snapshot`](LocalCollector::snapshot)。

use sysinfo::{Disks, Networks, System};

use crate::monitor::{DiskUsage, StatsSnapshot};

/// 本机指标采集器:跨采样复用 `sysinfo` 句柄(CPU% 依赖两次刷新间的增量)。
pub struct LocalCollector {
    sys: System,
    networks: Networks,
    disks: Disks,
}

impl Default for LocalCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalCollector {
    /// 新建并做一次基线刷新(让首帧的 CPU% 有上一拍可比)。
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_cpu_all();
        Self {
            sys,
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
        }
    }

    /// 采一帧本机快照(结构与远端一致;net 为累计字节,留给 `from_samples` 差分速率)。
    pub fn snapshot(&mut self) -> StatsSnapshot {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.networks.refresh(true);
        self.disks.refresh(true);

        let mut snap = StatsSnapshot {
            // sysinfo 直接给出全局 CPU%(基于上次刷新到现在的增量),走 direct 通道。
            cpu_percent_direct: Some(self.sys.global_cpu_usage()),
            cpu_cores: self.sys.cpus().len() as u32,
            // sysinfo 0.30+ 内存单位是字节,换算成 KB 与远端对齐。
            mem_total_kb: self.sys.total_memory() / 1024,
            mem_available_kb: self.sys.available_memory() / 1024,
            swap_total_kb: self.sys.total_swap() / 1024,
            swap_free_kb: self.sys.free_swap() / 1024,
            load1: System::load_average().one as f32,
            uptime_secs: System::uptime() as f64,
            os: System::long_os_version()
                .or_else(System::name)
                .unwrap_or_default(),
            ..Default::default()
        };

        // 网络:累计收发字节(跨接口求和),由 from_samples 差分成速率。
        let mut rx = 0u64;
        let mut tx = 0u64;
        for (_name, data) in &self.networks {
            rx = rx.saturating_add(data.total_received());
            tx = tx.saturating_add(data.total_transmitted());
        }
        snap.net_rx_bytes = rx;
        snap.net_tx_bytes = tx;

        // 磁盘:字节 → KB;跳过 0 容量伪卷,按容量降序、按挂载点去重。
        let mut disks: Vec<DiskUsage> = Vec::new();
        for disk in &self.disks {
            let total_kb = disk.total_space() / 1024;
            if total_kb == 0 {
                continue;
            }
            let avail_kb = disk.available_space() / 1024;
            disks.push(DiskUsage {
                mount: disk.mount_point().to_string_lossy().to_string(),
                total_kb,
                used_kb: total_kb.saturating_sub(avail_kb),
            });
        }
        disks.sort_by(|a, b| b.total_kb.cmp(&a.total_kb));
        disks.dedup_by(|a, b| a.mount == b.mount);
        if let Some(primary) = disks.first() {
            snap.disk_total_kb = primary.total_kb;
            snap.disk_used_kb = primary.used_kb;
        }
        snap.disks = disks;

        snap
    }
}
