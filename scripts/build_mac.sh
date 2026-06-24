#!/usr/bin/env bash
# 在 macOS 上编译 Conch 桌面版 (Flutter + Rust) release,并实现「零环境交付」:
# 把 adb 内嵌进 Conch.app/Contents/Resources/adb,ad-hoc 重新签名,最后压成 zip。
#
# 干净的目标 Mac(没装 Android SDK / Homebrew adb,且从 Finder 启动只有最小 PATH)
# 因此也能连安卓:程序优先用内嵌 adb(见 core/src/adb_bin.rs)。内嵌的 adb 是
# universal(x86_64+arm64)且只依赖系统库,Intel / Apple Silicon 双双即用。
#
# 用法(仓库根或任意目录均可):
#   bash scripts/build_mac.sh            # 全量:flutter build + 内嵌 + 签名 + zip
#   SKIP_BUILD=1 bash scripts/build_mac.sh   # 跳过 flutter build,只对已有 .app 内嵌+打包

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/desktop"
APP_NAME="Conch"
REL="$DESKTOP/build/macos/Build/Products/Release"
APP="$REL/$APP_NAME.app"
DIST="$ROOT/dist"

echo "== repo:    $ROOT"
echo "== desktop: $DESKTOP"

# 1) 编译 release(可用 SKIP_BUILD=1 复用已有产物快速迭代)
if [ "${SKIP_BUILD:-0}" != "1" ]; then
  ( cd "$DESKTOP" && flutter build macos --release )
fi
[ -d "$APP" ] || { echo "!! 未找到 $APP(先 flutter build macos,或别设 SKIP_BUILD)" >&2; exit 1; }
echo "== app:     $APP"

# 2) 取得内嵌用的 adb(缓存命中即用,否则下载官方 platform-tools)
ADB_DIR="$(bash "$ROOT/scripts/fetch_adb.sh" darwin | tail -1)"
SRC_ADB="$ADB_DIR/adb"
[ -x "$SRC_ADB" ] || { echo "!! adb 不可用: $SRC_ADB" >&2; exit 1; }

# 3) 拷进 .app 的 Resources/adb,并保证可执行
DST_DIR="$APP/Contents/Resources/adb"
mkdir -p "$DST_DIR"
cp -f "$SRC_ADB" "$DST_DIR/adb"
chmod +x "$DST_DIR/adb"
echo "== embed:   $DST_DIR/adb"

# 4) 签名:先签内嵌 adb,再整包 deep 重签(加文件会破坏原签名封印,必须重签)
if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$DST_DIR/adb"
  codesign --force --deep --sign - "$APP"
  echo "== signed:  ad-hoc (adb + app)"
fi

# 5) 自检:内嵌 adb 能跑 + 仅依赖系统库(否则干净机会缺库)。
#    universal 二进制的 otool -L 会有 "(architecture xxx):" 头行(不缩进),
#    依赖项是缩进行,故只看缩进行,避免把头行误判成非系统依赖。
echo "== verify embedded adb:"
"$DST_DIR/adb" version | sed 's/^/     /'
NONSYS="$(otool -L "$DST_DIR/adb" | grep -E '^[[:space:]]+/' | grep -vE '/usr/lib/|/System/' || true)"
if [ -n "$NONSYS" ]; then
  echo "  [WARN] 内嵌 adb 依赖了非系统库,干净机可能缺库:" >&2
  echo "$NONSYS" >&2
else
  echo "     deps OK(仅 /usr/lib 与 /System 系统库,干净机可直接运行)"
fi

# 6) 压成交付 zip
mkdir -p "$DIST"
ZIP="$DIST/Conch-macos.zip"
rm -f "$ZIP"
( cd "$REL" && ditto -c -k --sequesterRsrc --keepParent "$APP_NAME.app" "$ZIP" )
SIZE=$(du -h "$ZIP" | cut -f1)
echo "ZIP_READY $ZIP ($SIZE)"
