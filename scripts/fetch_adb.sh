#!/usr/bin/env bash
# 确保「内嵌用的 adb」就位,并打印其所在目录(供打包脚本拷贝)。
#
# 用法:  fetch_adb.sh <darwin|windows|linux>
# 输出:  最后一行 = 含 adb 的目录(stdout);进度信息走 stderr。
#
# 策略(就近优先,保证离线也能出包):
#   1) 仓库缓存 vendor/adb/<plat>/ 已有 adb 则直接用;
#   2) 否则下载 Google 官方 platform-tools(自带、纯系统依赖,干净机即可运行),
#      抽出 adb(Windows 额外抽 AdbWinApi.dll / AdbWinUsbApi.dll)塞进缓存;
#   3) darwin 兜底:从本机 PATH / Homebrew 拷一份(本机 adb 是 universal 二进制)。
#
# 下载器遵循工作区规范:优先 surge,缺失回退 aria2c,再回退 curl。

set -euo pipefail

PLAT="${1:?usage: fetch_adb.sh <darwin|windows|linux>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CACHE="$ROOT/vendor/adb/$PLAT"
log() { echo "[fetch_adb] $*" >&2; }

case "$PLAT" in
  darwin)  ADB_BIN="adb";     ZIP_OS="darwin"  ;;
  linux)   ADB_BIN="adb";     ZIP_OS="linux"   ;;
  windows) ADB_BIN="adb.exe"; ZIP_OS="windows" ;;
  *) log "未知平台: $PLAT"; exit 2 ;;
esac

# 1) 缓存命中
if [ -f "$CACHE/$ADB_BIN" ]; then
  log "命中缓存: $CACHE/$ADB_BIN"
  echo "$CACHE"
  exit 0
fi

mkdir -p "$CACHE"

# 下载助手:download <url> <out_file>
download() {
  local url="$1" out="$2"
  if command -v surge >/dev/null 2>&1; then
    log "surge 下载: $url"
    surge "$url" -o "$(dirname "$out")" 2>/dev/null && return 0 || true
    # surge 命名可能不同,失败则继续回退
  fi
  if command -v aria2c >/dev/null 2>&1; then
    log "aria2c 下载: $url"
    aria2c -q -x8 -s8 -o "$(basename "$out")" -d "$(dirname "$out")" "$url" && return 0 || true
  fi
  log "curl 下载: $url"
  curl -fL --retry 3 -o "$out" "$url"
}

ZIP_URL="https://dl.google.com/android/repository/platform-tools-latest-${ZIP_OS}.zip"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if download "$ZIP_URL" "$TMP/pt.zip" && [ -s "$TMP/pt.zip" ]; then
  log "解压 platform-tools..."
  unzip -q -o "$TMP/pt.zip" -d "$TMP"
  cp "$TMP/platform-tools/$ADB_BIN" "$CACHE/$ADB_BIN"
  if [ "$PLAT" = "windows" ]; then
    cp "$TMP/platform-tools/AdbWinApi.dll"    "$CACHE/" 2>/dev/null || true
    cp "$TMP/platform-tools/AdbWinUsbApi.dll" "$CACHE/" 2>/dev/null || true
  fi
  chmod +x "$CACHE/$ADB_BIN" 2>/dev/null || true
  log "已写入缓存: $CACHE"
  echo "$CACHE"
  exit 0
fi

# 3) darwin 兜底:本机已装的 adb(universal,够用)
if [ "$PLAT" = "darwin" ]; then
  HOST_ADB="$(command -v adb || true)"
  [ -z "$HOST_ADB" ] && [ -x /opt/homebrew/share/android-commandlinetools/platform-tools/adb ] \
    && HOST_ADB=/opt/homebrew/share/android-commandlinetools/platform-tools/adb
  if [ -n "$HOST_ADB" ] && [ -x "$HOST_ADB" ]; then
    log "下载失败,改用本机 adb: $HOST_ADB"
    cp "$HOST_ADB" "$CACHE/adb"
    chmod +x "$CACHE/adb"
    echo "$CACHE"
    exit 0
  fi
fi

log "无法获取 adb($PLAT):下载失败且无本机兜底"
exit 1
