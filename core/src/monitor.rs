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
         echo {m}; df -kP 2>/dev/null; \
         echo {m}; nproc 2>/dev/null; \
         echo {m}; uname -sr 2>/dev/null"
    )
}

/// First line emitted by [`windows_stats_command`]; lets us confirm the output
/// really came from the Windows collector (and not a shell error).
pub const WINDOWS_STATS_SENTINEL: &str = "RSHELL_WIN";

/// PowerShell one-liner that snapshots Windows resource usage via CIM in a single
/// round-trip. Emits `KEY=VALUE` lines parsed by [`parse_windows_snapshot`].
///
/// Quotes are single only (no `"`), to avoid Windows-OpenSSH exec quote mangling.
/// `LoadPercentage` is an instantaneous CPU%, so the snapshot carries
/// `cpu_percent_direct` rather than jiffies. Memory/paging are in KB; disk bytes
/// are converted to KB; network counters are cumulative bytes (diffed to rates).
pub fn windows_stats_command() -> String {
    concat!(
        "$ErrorActionPreference='SilentlyContinue'; ",
        // Emit UTF-8 so a localized OS Caption (e.g. Chinese) isn't mangled by the
        // remote console's legacy code page when read over the SSH exec channel.
        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; ",
        "$o=Get-CimInstance Win32_OperatingSystem; ",
        "$cs=Get-CimInstance Win32_ComputerSystem; ",
        "$cpu=(Get-CimInstance Win32_Processor|Measure-Object LoadPercentage -Average).Average; ",
        "$sys=$env:SystemDrive; ",
        "$disks=Get-CimInstance Win32_LogicalDisk|Where-Object {$_.DriveType -eq 3}; ",
        "$d=$disks|Where-Object {$_.DeviceID -eq $sys}; ",
        "$n=Get-CimInstance Win32_PerfRawData_Tcpip_NetworkInterface|Where-Object ",
        "{$_.Name -notlike '*Loopback*' -and $_.Name -notlike '*isatap*'}; ",
        "$rx=($n|Measure-Object BytesReceivedPersec -Sum).Sum; ",
        "$tx=($n|Measure-Object BytesSentPersec -Sum).Sum; ",
        "$up=((Get-Date)-$o.LastBootUpTime).TotalSeconds; ",
        "Write-Output 'RSHELL_WIN'; ",
        "Write-Output ('CPU=' + [int]$cpu); ",
        "Write-Output ('CORES=' + [int]$cs.NumberOfLogicalProcessors); ",
        "Write-Output ('MEMTOTAL=' + [int64]$o.TotalVisibleMemorySize); ",
        "Write-Output ('MEMFREE=' + [int64]$o.FreePhysicalMemory); ",
        "Write-Output ('SWAPTOTAL=' + [int64]$o.SizeStoredInPagingFiles); ",
        "Write-Output ('SWAPFREE=' + [int64]$o.FreeSpaceInPagingFiles); ",
        "Write-Output ('DISKTOTAL=' + [int64]($d.Size/1024)); ",
        "Write-Output ('DISKFREE=' + [int64]($d.FreeSpace/1024)); ",
        "Write-Output ('NETRX=' + [int64]$rx); ",
        "Write-Output ('NETTX=' + [int64]$tx); ",
        "Write-Output ('UPTIME=' + [int64]$up); ",
        // One DISK line per local fixed drive (DeviceID|totalKB|freeKB) feeds the
        // multi-disk view; DISKTOTAL/DISKFREE above stay the system drive headline.
        "foreach($x in $disks){Write-Output ('DISK=' + $x.DeviceID + '|' + ",
        "[int64]($x.Size/1024) + '|' + [int64]($x.FreeSpace/1024))}; ",
        "Write-Output ('OS=' + $o.Caption)",
    )
    .to_string()
}

/// Parse the `KEY=VALUE` output of [`windows_stats_command`] into a snapshot.
/// Unknown lines are ignored; missing keys default to 0 / empty.
pub fn parse_windows_snapshot(raw: &str) -> StatsSnapshot {
    let mut snap = StatsSnapshot::default();
    let mut mem_total = 0u64;
    let mut mem_free = 0u64;
    let mut swap_total = 0u64;
    let mut swap_free = 0u64;
    let mut disk_total = 0u64;
    let mut disk_free = 0u64;

    for line in raw.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "DISK" => {
                // DeviceID|totalKB|freeKB, e.g. "C:|123837528|361600".
                let parts: Vec<&str> = value.split('|').collect();
                if parts.len() == 3 {
                    if let (Ok(total_kb), Ok(free_kb)) =
                        (parts[1].parse::<u64>(), parts[2].parse::<u64>())
                    {
                        if total_kb > 0 {
                            snap.disks.push(DiskUsage {
                                mount: parts[0].to_string(),
                                total_kb,
                                used_kb: total_kb.saturating_sub(free_kb),
                            });
                        }
                    }
                }
            }
            "CPU" => {
                snap.cpu_percent_direct =
                    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0));
            }
            "CORES" => snap.cpu_cores = value.parse().unwrap_or(0),
            "MEMTOTAL" => mem_total = value.parse().unwrap_or(0),
            "MEMFREE" => mem_free = value.parse().unwrap_or(0),
            "SWAPTOTAL" => swap_total = value.parse().unwrap_or(0),
            "SWAPFREE" => swap_free = value.parse().unwrap_or(0),
            "DISKTOTAL" => disk_total = value.parse().unwrap_or(0),
            "DISKFREE" => disk_free = value.parse().unwrap_or(0),
            "NETRX" => snap.net_rx_bytes = value.parse().unwrap_or(0),
            "NETTX" => snap.net_tx_bytes = value.parse().unwrap_or(0),
            "UPTIME" => snap.uptime_secs = value.parse().unwrap_or(0.0),
            "OS" => snap.os = value.to_string(),
            _ => {}
        }
    }

    snap.mem_total_kb = mem_total;
    snap.mem_available_kb = mem_free;
    snap.swap_total_kb = swap_total;
    snap.swap_free_kb = swap_free;
    snap.disk_total_kb = disk_total;
    snap.disk_used_kb = disk_total.saturating_sub(disk_free);

    // Largest local drive first; cap to keep the panel tidy.
    snap.disks.sort_by(|a, b| b.total_kb.cmp(&a.total_kb));
    snap.disks.truncate(MAX_DISKS);
    // Fall back to the biggest drive as headline if the system drive was missing.
    if snap.disk_total_kb == 0 {
        if let Some(primary) = primary_disk(&snap.disks) {
            snap.disk_total_kb = primary.total_kb;
            snap.disk_used_kb = primary.used_kb;
        }
    }
    // Windows has no 1-minute load average; leave load1 at 0.
    snap
}

/// Cumulative CPU jiffies parsed from the aggregate `cpu` line of `/proc/stat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub total: u64,
}

/// One mounted filesystem worth showing in the monitor (a real disk / volume).
/// `mount` is the human label: a Unix mount point (`/`, `/data`) or a Windows
/// drive (`C:`). Sizes are in KB.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiskUsage {
    pub mount: String,
    pub total_kb: u64,
    pub used_kb: u64,
}

impl DiskUsage {
    /// Used percentage (0.0..=100.0); 0 when total is unknown.
    pub fn percent(&self) -> f32 {
        if self.total_kb == 0 {
            0.0
        } else {
            (self.used_kb as f32 / self.total_kb as f32 * 100.0).clamp(0.0, 100.0)
        }
    }
}

/// A raw, point-in-time snapshot. Absolute counters here are converted to rates
/// once paired with a previous snapshot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatsSnapshot {
    pub cpu: Option<CpuTimes>,
    /// Pre-computed CPU% (0..=100) for platforms that report it directly instead
    /// of cumulative jiffies (e.g. Windows via CIM `LoadPercentage`). When `cpu`
    /// jiffies are present they win; this is the fallback.
    pub cpu_percent_direct: Option<f32>,
    pub mem_total_kb: u64,
    pub mem_available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
    pub load1: f32,
    pub uptime_secs: f64,
    /// Cumulative bytes received / transmitted across all real interfaces.
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    /// Primary disk (largest real filesystem) — kept for the compact display and
    /// backward compatibility. Derived from `disks`.
    pub disk_total_kb: u64,
    pub disk_used_kb: u64,
    /// All real disks / volumes worth showing (sorted largest first).
    pub disks: Vec<DiskUsage>,
    pub cpu_cores: u32,
    pub os: String,
}

impl StatsSnapshot {
    /// True when the snapshot carries no usable data — used to detect that a
    /// platform-specific collector produced nothing (e.g. the Linux command ran
    /// on a non-Linux host and yielded only empty sections).
    pub fn is_empty(&self) -> bool {
        self.cpu.is_none()
            && self.cpu_percent_direct.is_none()
            && self.mem_total_kb == 0
            && self.disk_total_kb == 0
            && self.uptime_secs == 0.0
    }
}

/// Which OS family a connection's stats are collected from. Detected once per
/// connection, then cached to avoid re-probing every sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsPlatform {
    Unix,
    /// macOS / jailbroken iOS (`sysctl` + `vm_stat`, no `/proc`).
    Darwin,
    Windows,
}

/// Sentinel first line of [`darwin_stats_command`].
pub const DARWIN_STATS_SENTINEL: &str = "RSHELL_DARWIN";

/// One-shot Darwin / jailbroken-iOS collector. Emits `KEY=VALUE` for
/// [`parse_darwin_snapshot`]. Prefers Procursus `python3` under `/var/jb`;
/// if missing (common on rootless JB), falls back to pure shell — never `exit 1`.
///
/// Python path **must not** use `subprocess` `shell=True` (no `/bin/sh` on many
/// rootless installs). Memory: used ≈ wired+active+compressor; avail ≈
/// free+speculative+inactive+purgeable. No fake CPU% from loadavg.
fn darwin_stats_shell_fallback() -> &'static str {
    // Prefer tools on `/var/jb` PATH (set by caller). No awk/python — only
    // sysctl / vm_stat / df / pagesize / tr / date, which rootless JB usually has.
    concat!(
        "echo RSHELL_DARWIN; ",
        "page=$(pagesize 2>/dev/null || echo 16384); page=${page:-16384}; ",
        "memtotal=$(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1024 )); ",
        "cores=$(sysctl -n hw.ncpu 2>/dev/null || echo 0); ",
        "loadraw=$(sysctl -n vm.loadavg 2>/dev/null | tr -cd '0-9. '); ",
        "set -- $loadraw; load=${1:-0}; ",
        "bt=$(sysctl -n kern.boottime 2>/dev/null || true); ",
        "set -- $(printf '%s' \"$bt\" | tr -cd '0-9 '); sec=${1:-0}; ",
        "now=$(date +%s 2>/dev/null || echo 0); ",
        "up=0; ",
        "if [ \"$sec\" -gt 0 ] 2>/dev/null; then up=$((now - sec)); fi; ",
        "osname=$(uname -sr 2>/dev/null || echo Darwin); ",
        "vs=$(vm_stat 2>/dev/null || true); ",
        "pg(){ ",
        "  k=\"$1\"; out=; ",
        "  out=$(printf '%s\\n' \"$vs\" | while IFS= read -r line; do ",
        "    case \"$line\" in *\"$k\"*) printf '%s' \"$line\" | tr -cd '0-9'; echo; break;; esac; ",
        "  done); ",
        "  echo \"${out:-0}\"; ",
        "}; ",
        "wired=$(pg 'Pages wired'); wired=${wired:-0}; ",
        "active=$(pg 'Pages active'); active=${active:-0}; ",
        "inactive=$(pg 'Pages inactive'); inactive=${inactive:-0}; ",
        "spec=$(pg 'Pages speculative'); spec=${spec:-0}; ",
        "free=$(pg 'Pages free'); free=${free:-0}; ",
        "purge=$(pg 'Pages purgeable'); purge=${purge:-0}; ",
        "comp=$(pg 'Pages occupied by compressor'); comp=${comp:-0}; ",
        "used=$(( (wired + active + comp) * page / 1024 )); ",
        "avail=$(( (free + spec + inactive + purge) * page / 1024 )); ",
        "if [ \"$memtotal\" -gt 0 ] 2>/dev/null && [ $((avail + used)) -gt \"$memtotal\" ]; then ",
        "  avail=$((memtotal - used)); ",
        "fi; ",
        "if [ \"$avail\" -lt 0 ] 2>/dev/null; then avail=0; fi; ",
        "echo CORES=$cores; ",
        "echo MEMTOTAL=$memtotal; ",
        "echo MEMUSED=$used; ",
        "echo MEMAVAIL=$avail; ",
        "echo LOAD=$load; ",
        "echo UPTIME=$up; ",
        "echo OS=$osname; ",
        // Avoid `*[!0-9]*` (zsh EXTENDED_GLOB / hist quirks). Prefer POSIX `[ -eq ]`.
        "df -kP 2>/dev/null | while IFS= read -r line; do ",
        "  set -- $line; ",
        "  [ \"$#\" -ge 6 ] || continue; ",
        "  dtotal=$2; dused=$3; ",
        "  shift 5; dmount=$1; ",
        "  [ \"$dtotal\" -eq \"$dtotal\" ] 2>/dev/null || continue; ",
        "  [ \"$dtotal\" -gt 0 ] 2>/dev/null || continue; ",
        "  case \"$dmount\" in /dev|/dev/*) continue;; esac; ",
        "  dfree=$((dtotal - dused)); ",
        "  if [ \"$dfree\" -lt 0 ] 2>/dev/null; then dfree=0; fi; ",
        "  printf 'DISK=%s|%s|%s\\n' \"$dmount\" \"$dtotal\" \"$dfree\"; ",
        "done; ",
        "exit 0"
    )
}

pub fn darwin_stats_command() -> String {
    // Prefer Python when available; otherwise pure shell — NEVER exit 1.
    // Python via `-c` with embedded newlines; no single quotes in the payload.
    let mut cmd = String::from(concat!(
        "export PATH=/var/jb/usr/bin:/var/jb/bin:/var/jb/usr/sbin:/var/jb/sbin:",
        "/iosbinpack64/usr/bin:/iosbinpack64/bin:/usr/bin:/bin:/usr/sbin:/sbin; ",
        // iOS OpenSSH often uses zsh as the login shell. zsh does NOT split
        // unquoted `$var` by default, so `set -- $line` / `set -- $loadraw`
        // become a single field and the df parser skips every row (Disk 0/0).
        // Enable SH_WORD_SPLIT for this snippet; no-op on bash/dash.
        "[ -n \"$ZSH_VERSION\" ] && setopt SH_WORD_SPLIT 2>/dev/null; ",
        "PY=/var/jb/usr/bin/python3; ",
        "[ -x \"$PY\" ] || PY=$(command -v python3 2>/dev/null); ",
        "[ -n \"$PY\" ] || PY=$(command -v python 2>/dev/null); ",
        "[ -n \"$PY\" ] || { ",
    ));
    cmd.push_str(darwin_stats_shell_fallback());
    cmd.push_str("; }; ");
    cmd.push_str(concat!(
        "\"$PY\" -c \"",
        "import re,time,subprocess,os\n",
        "P=['/var/jb/usr/sbin','/var/jb/usr/bin','/var/jb/bin','/var/jb/sbin','/usr/sbin','/usr/bin','/bin','/sbin']\n",
        "def which(n):\n",
        " for d in P:\n",
        "  p=os.path.join(d,n)\n",
        "  if os.path.isfile(p) and os.access(p,os.X_OK): return p\n",
        " return n\n",
        "def S(argv):\n",
        " try: return subprocess.check_output(argv,stderr=subprocess.DEVNULL).decode('utf-8','replace')\n",
        " except Exception: return ''\n",
        "sysctl=which('sysctl'); vmstat=which('vm_stat'); uname=which('uname'); dfb=which('df'); psz=which('pagesize')\n",
        "page=int((S([psz]).strip() or '16384'))\n",
        "memtotal=int(S([sysctl,'-n','hw.memsize']).strip() or '0')//1024\n",
        "vs=S([vmstat])\n",
        "def pg(n):\n",
        " m=re.search(n+r'[^0-9]*([0-9]+)',vs)\n",
        " return int(m.group(1)) if m else 0\n",
        "wired=pg('Pages wired')*page//1024\n",
        "active=pg('Pages active')*page//1024\n",
        "inactive=pg('Pages inactive')*page//1024\n",
        "spec=pg('Pages speculative')*page//1024\n",
        "free=pg('Pages free')*page//1024\n",
        "purge=pg('Pages purgeable')*page//1024\n",
        "comp=pg('Pages occupied by compressor')*page//1024\n",
        "used=wired+active+comp\n",
        "avail=free+spec+inactive+purge\n",
        "if avail+used>memtotal and memtotal>0: avail=max(memtotal-used,0)\n",
        "cores=int(S([sysctl,'-n','hw.ncpu']).strip() or '0')\n",
        "lm=re.search(r'([0-9]+\\.[0-9]+)', S([sysctl,'-n','vm.loadavg']))\n",
        "load=float(lm.group(1)) if lm else 0.0\n",
        "bm=re.search(r'sec\\s*=\\s*([0-9]+)', S([sysctl,'-n','kern.boottime']))\n",
        "up=int(time.time())-int(bm.group(1)) if bm else 0\n",
        "osname=S([uname,'-sr']).strip()\n",
        "print('RSHELL_DARWIN')\n",
        // No CPU= line: loadavg≠CPU%; UI keeps CPU at 0 until a real sampler exists.
        "print('CORES=%d'%cores)\n",
        "print('MEMTOTAL=%d'%memtotal)\n",
        "print('MEMUSED=%d'%used)\n",
        "print('MEMAVAIL=%d'%avail)\n",
        "print('LOAD=%.2f'%load)\n",
        "print('UPTIME=%d'%up)\n",
        "print('OS='+osname)\n",
        "df=S([dfb,'-kP'])\n",
        "for line in df.splitlines()[1:]:\n",
        " p=line.split()\n",
        " if len(p)>=6 and p[1].isdigit():\n",
        "  total,used_kb,mount=int(p[1]),int(p[2]),p[5]\n",
        "  if total>0 and not mount.startswith('/dev'):\n",
        "   print('DISK=%s|%d|%d'%(mount,total,max(total-used_kb,0)))\n",
        "\"",
    ));
    cmd
}

/// Parse [`darwin_stats_command`] `KEY=VALUE` output into a snapshot.
pub fn parse_darwin_snapshot(raw: &str) -> StatsSnapshot {
    let mut snap = StatsSnapshot::default();
    let mut mem_total = 0u64;
    let mut mem_used = 0u64;
    let mut mem_avail = 0u64;
    let mut disks: Vec<DiskUsage> = Vec::new();

    if !raw.contains(DARWIN_STATS_SENTINEL) {
        return snap;
    }

    for line in raw.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "CPU" => {
                if let Ok(v) = value.parse::<f32>() {
                    snap.cpu_percent_direct = Some(v.clamp(0.0, 100.0));
                }
            }
            "CORES" => {
                snap.cpu_cores = value.parse().unwrap_or(0);
            }
            "MEMTOTAL" => {
                mem_total = value.parse().unwrap_or(0);
            }
            "MEMUSED" => {
                mem_used = value.parse().unwrap_or(0);
            }
            "MEMAVAIL" => {
                mem_avail = value.parse().unwrap_or(0);
            }
            "LOAD" => {
                snap.load1 = value.parse().unwrap_or(0.0);
            }
            "UPTIME" => {
                snap.uptime_secs = value.parse::<f64>().unwrap_or(0.0);
            }
            "OS" => {
                snap.os = value.to_string();
            }
            "DISK" => {
                let parts: Vec<&str> = value.split('|').collect();
                if parts.len() == 3 {
                    if let (Ok(total_kb), Ok(free_kb)) =
                        (parts[1].parse::<u64>(), parts[2].parse::<u64>())
                    {
                        let used_kb = total_kb.saturating_sub(free_kb);
                        disks.push(DiskUsage {
                            mount: parts[0].to_string(),
                            total_kb,
                            used_kb,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    snap.mem_total_kb = mem_total;
    if mem_avail > 0 {
        snap.mem_available_kb = mem_avail;
    } else if mem_total > 0 && mem_used <= mem_total {
        snap.mem_available_kb = mem_total.saturating_sub(mem_used);
    }

    disks.sort_by(|a, b| b.total_kb.cmp(&a.total_kb));
    disks.truncate(MAX_DISKS);
    if let Some(primary) = primary_disk(&disks) {
        snap.disk_total_kb = primary.total_kb;
        snap.disk_used_kb = primary.used_kb;
    }
    snap.disks = disks;
    snap
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
    /// Primary disk (largest real filesystem): percent / used / total.
    pub disk_percent: f32,
    pub disk_used_kb: u64,
    pub disk_total_kb: u64,
    /// All real disks / volumes (sorted largest first) for the multi-disk view.
    pub disks: Vec<DiskUsage>,
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

    snap.disks = parse_disks(section(&parts, 5));
    if let Some(primary) = primary_disk(&snap.disks) {
        snap.disk_total_kb = primary.total_kb;
        snap.disk_used_kb = primary.used_kb;
    }

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

/// Most real filesystems we ever show in the monitor; keeps a server with dozens
/// of mounts (or an Android device's many partitions) from cluttering the panel.
const MAX_DISKS: usize = 8;

/// KB per `df` block, read from the header's size column. POSIX `-P` mandates
/// 512-byte blocks while `-k` mandates 1024 — and they conflict: Linux coreutils
/// honours `-k` ("1024-blocks"), but Android's toybox honours `-P` ("512-blocks")
/// even when `-k` is also passed. Reading the header makes sizes correct on both
/// (without this, Android disks read 2× too large).
fn header_block_kb(header: &str) -> f64 {
    let lower = header.to_ascii_lowercase();
    if lower.contains("512-blocks") {
        0.5
    } else {
        // "1024-blocks" / "1k-blocks" / anything unexpected → assume KB.
        1.0
    }
}

/// True for a filesystem worth surfacing to the user: a real, block-backed (or
/// user-facing fuse) volume — not a pseudo / virtual / system mount. The denylist
/// covers Linux servers and Android (tmpfs, cgroups, apex squashfs images, the
/// read-only ramdisk root's siblings, MIUI vendor partitions, etc.).
fn is_real_disk(source: &str, mount: &str) -> bool {
    const PSEUDO_SOURCES: &[&str] = &[
        "tmpfs", "devtmpfs", "ramfs", "none", "udev", "proc", "sysfs", "cgroup",
        "cgroup2", "overlay", "squashfs", "mqueue", "devpts", "debugfs",
        "tracefs", "securityfs", "pstore", "efivarfs", "configfs",
        "binfmt_misc", "fusectl", "autofs", "magisk", "shm",
    ];
    // Compare on the bare source name (last path component handles `/dev/...`).
    let src = source.rsplit('/').next().unwrap_or(source).to_ascii_lowercase();
    if PSEUDO_SOURCES.iter().any(|p| src == *p) {
        return false;
    }

    const SKIP_PREFIXES: &[&str] = &[
        "/dev", "/proc", "/sys", "/run", "/apex", "/snap", "/linkerconfig",
        "/metadata", "/debug_ramdisk", "/data_mirror", "/mnt", "/cache",
        "/system/app", "/persist",
    ];
    !SKIP_PREFIXES
        .iter()
        .any(|p| mount == *p || mount.starts_with(&format!("{p}/")))
}

/// Parse `df -kP` output into every real, user-relevant filesystem (sorted
/// largest first, capped at [`MAX_DISKS`]). Bind / fuse mirrors of the same
/// storage are de-duplicated — e.g. Android's `/storage/emulated` (`/dev/fuse`)
/// mirrors `/data`, so only one entry survives.
fn parse_disks(section: &str) -> Vec<DiskUsage> {
    // The combined stats command prefixes each section with an `echo` marker, so
    // this text starts with a blank line — find the real `df` header (first
    // non-empty line) and read the block unit from it before parsing rows.
    let mut block_kb = 1.0;
    let mut header_seen = false;

    let mut disks: Vec<DiskUsage> = Vec::new();
    for line in section.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !header_seen {
            block_kb = header_block_kb(line);
            header_seen = true;
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Filesystem 1024-blocks Used Available Capacity Mounted-on
        if cols.len() < 6 {
            continue;
        }
        let source = cols[0];
        // The mount point is everything after the capacity column (it may contain
        // spaces); the four numeric columns in between are fixed-position.
        let mount = cols[5..].join(" ");
        let (Ok(blocks), Ok(used_blocks)) =
            (cols[1].parse::<u64>(), cols[2].parse::<u64>())
        else {
            continue;
        };
        if blocks == 0 || !is_real_disk(source, &mount) {
            continue;
        }
        let total_kb = (blocks as f64 * block_kb) as u64;
        let used_kb = (used_blocks as f64 * block_kb) as u64;
        // Skip a mirror of a volume we already recorded (identical size + usage).
        if disks
            .iter()
            .any(|d| d.total_kb == total_kb && d.used_kb == used_kb)
        {
            continue;
        }
        disks.push(DiskUsage {
            mount,
            total_kb,
            used_kb,
        });
    }

    disks.sort_by(|a, b| b.total_kb.cmp(&a.total_kb));
    disks.truncate(MAX_DISKS);
    disks
}

/// The headline disk for the compact display: the largest real filesystem
/// (`/data` on Android, usually `/` or the biggest data volume on Linux).
fn primary_disk(disks: &[DiskUsage]) -> Option<&DiskUsage> {
    disks.iter().max_by_key(|d| d.total_kb)
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
            disks: current.disks.clone(),
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
        } else if let Some(direct) = current.cpu_percent_direct {
            // Platforms without cumulative jiffies (Windows) report CPU% directly,
            // so it is available from the very first sample.
            stats.cpu_percent = direct.clamp(0.0, 100.0);
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
    fn parses_windows_snapshot_output() {
        let raw = "RSHELL_WIN\nCPU=41\nCORES=8\nMEMTOTAL=16714340\nMEMFREE=9753752\n\
                   SWAPTOTAL=11534336\nSWAPFREE=11270212\nDISKTOTAL=123837528\nDISKFREE=361600\n\
                   NETRX=64512920\nNETTX=14837250\nUPTIME=538531\n\
                   OS=Microsoft Windows 11 专业工作站版 Insider Preview\n";
        let snap = parse_windows_snapshot(raw);
        assert_eq!(snap.cpu, None);
        assert_eq!(snap.cpu_percent_direct, Some(41.0));
        assert_eq!(snap.cpu_cores, 8);
        assert_eq!(snap.mem_total_kb, 16_714_340);
        assert_eq!(snap.mem_available_kb, 9_753_752);
        assert_eq!(snap.swap_total_kb, 11_534_336);
        assert_eq!(snap.swap_free_kb, 11_270_212);
        assert_eq!(snap.disk_total_kb, 123_837_528);
        assert_eq!(snap.disk_used_kb, 123_837_528 - 361_600);
        assert_eq!(snap.net_rx_bytes, 64_512_920);
        assert_eq!(snap.net_tx_bytes, 14_837_250);
        assert!((snap.uptime_secs - 538_531.0).abs() < 1e-6);
        assert!(snap.os.contains("Windows 11"));
        assert!(!snap.is_empty());
    }

    #[test]
    fn windows_snapshot_derives_mem_and_direct_cpu_on_first_sample() {
        let raw = "RSHELL_WIN\nCPU=41\nCORES=8\nMEMTOTAL=1000\nMEMFREE=250\n\
                   DISKTOTAL=2000\nDISKFREE=500\nUPTIME=10\nOS=Windows\n";
        let cur = parse_windows_snapshot(raw);
        // No previous sample, but Windows CPU% is available immediately.
        let stats = SystemStats::from_samples(None, &cur, 0.0);
        assert!((stats.cpu_percent - 41.0).abs() < 1e-3);
        // mem used = 1000 - 250 = 750 => 75%.
        assert_eq!(stats.mem_used_kb, 750);
        assert!((stats.mem_percent - 75.0).abs() < 1e-3);
        // disk used = 2000 - 500 = 1500 => 75%.
        assert_eq!(stats.disk_used_kb, 1500);
        assert!((stats.disk_percent - 75.0).abs() < 1e-3);
    }

    #[test]
    fn empty_snapshot_detected() {
        assert!(StatsSnapshot::default().is_empty());
        // A Linux command that ran on Windows yields only blank sections.
        let blank = format!(
            "{m}\n{m}\n{m}\n{m}\n{m}\n{m}\n{m}\n{m}\n",
            m = SECTION_MARKER
        );
        assert!(parse_snapshot(&blank).is_empty());
    }

    #[test]
    fn parses_android_df_512_blocks_dedupes_and_filters() {
        // Real Redmi / Android 11 `df -kP`: the header says 512-byte blocks, and
        // the list is full of pseudo / system mounts plus a fuse mirror of /data.
        // Leading blank line mimics the `echo MARKER` prefix in the real section.
        let section = "\n\
Filesystem            512-blocks     Used Available Capacity Mounted on
/dev/block/dm-3          5691488  5674456         0     100% /
tmpfs                    3867016     4200   3862816       1% /dev
/dev/block/mmcblk0p13      36616      272     34968       1% /metadata
/dev/block/dm-4          1302744  1298760         0     100% /vendor
/dev/block/dm-5           239424   238664         0     100% /product
magisk                   3867016     5496   3861520       1% /debug_ramdisk
/dev/block/mmcblk0p40     824208     9704    787976       2% /cache
/dev/block/mmcblk0p7     1644336  1219360    375176      77% /cust
/dev/block/mmcblk0p41  108632024 76866048  31553944      71% /data
/dev/block/loop16          81576    81520         0     100% /apex/com.android.art@1
/dev/fuse              108632024 76866048  31553944      71% /storage/emulated";

        let disks = parse_disks(section);
        let mounts: Vec<&str> = disks.iter().map(|d| d.mount.as_str()).collect();
        // Sorted largest first; pseudo/system mounts dropped; the /storage/emulated
        // fuse view is a mirror of /data and gets de-duplicated out.
        assert_eq!(mounts, vec!["/data", "/", "/cust", "/vendor", "/product"]);

        // 512-byte blocks: 108632024 × 512 / 1024 = 54_316_012 KB (~51.8 GiB), not 2×.
        let data = &disks[0];
        assert_eq!(data.total_kb, 54_316_012);
        assert_eq!(data.used_kb, 38_433_024);
        assert_eq!(primary_disk(&disks).unwrap().mount, "/data");
    }

    #[test]
    fn parses_windows_multi_disk() {
        let raw = "RSHELL_WIN\nCPU=10\nCORES=8\nMEMTOTAL=1000\nMEMFREE=500\n\
                   DISKTOTAL=2000\nDISKFREE=500\nUPTIME=10\n\
                   DISK=C:|2000|500\nDISK=D:|8000|2000\nOS=Windows\n";
        let snap = parse_windows_snapshot(raw);
        // Headline stays the system drive (DISKTOTAL/DISKFREE).
        assert_eq!(snap.disk_total_kb, 2000);
        assert_eq!(snap.disk_used_kb, 1500);
        // Multi-disk list sorted largest first: D: (8000) before C: (2000).
        assert_eq!(snap.disks.len(), 2);
        assert_eq!(snap.disks[0].mount, "D:");
        assert_eq!(snap.disks[0].total_kb, 8000);
        assert_eq!(snap.disks[0].used_kb, 6000);
        assert_eq!(snap.disks[1].mount, "C:");
    }

    #[test]
    fn formats_helpers() {
        assert_eq!(format_bytes_f64(512.0), "512 B");
        assert_eq!(format_rate(1536.0), "1.5 KB/s");
        assert_eq!(format_uptime(90_061.0), "1d 1h 1m");
        assert_eq!(format_kb_pair(5_100_000, 8_192_000), "4.9/7.8 GB");
        assert_eq!(format_kb_pair(512, 1000), "512.0/1000.0 KB");
    }

    #[test]
    fn parses_darwin_snapshot_output() {
        // No CPU= — iOS has no reliable cp_time; load stays in LOAD only.
        let raw = "RSHELL_DARWIN\nCORES=6\nMEMTOTAL=3848928\nMEMUSED=2540000\n\
                   MEMAVAIL=1200000\nLOAD=3.29\nUPTIME=273900\nOS=Darwin 22.2.0\n\
                   DISK=/private/var|200000000|150000000\nDISK=/|249879124|150232292\n";
        let snap = parse_darwin_snapshot(raw);
        assert!(!snap.is_empty());
        assert_eq!(snap.cpu_percent_direct, None);
        assert_eq!(snap.cpu_cores, 6);
        assert_eq!(snap.mem_total_kb, 3_848_928);
        assert_eq!(snap.mem_available_kb, 1_200_000);
        assert!((snap.load1 - 3.29).abs() < 1e-6);
        assert!((snap.uptime_secs - 273_900.0).abs() < 1e-6);
        assert_eq!(snap.os, "Darwin 22.2.0");
        assert!(snap.disks.len() >= 2);
        // Largest volume first (`/` 249M > `/private/var` 200M).
        assert_eq!(snap.disks[0].mount, "/");
        assert_eq!(snap.disk_total_kb, 249_879_124);
        let stats = SystemStats::from_samples(None, &snap, 0.0);
        // ~ (3848928-1200000)/3848928 ≈ 68.8%
        assert!((stats.mem_percent - 68.8).abs() < 0.2);
        assert_eq!(stats.cpu_percent, 0.0);
    }

    #[test]
    fn darwin_without_sentinel_is_empty() {
        assert!(parse_darwin_snapshot("CPU=10\nMEMTOTAL=100\n").is_empty());
    }

    #[test]
    fn darwin_stats_command_falls_back_without_exit_1() {
        let cmd = darwin_stats_command();
        assert!(cmd.contains("RSHELL_DARWIN"));
        assert!(cmd.contains("sysctl -n hw.memsize"));
        // Must not hard-fail when python3 is missing (broke iPhone monitor).
        assert!(!cmd.contains("RSHELL_DARWIN_NO_PY"));
        assert!(!cmd.contains("exit 1"));
    }
}
