<#
  在 Windows 上编译 Conch 桌面版 (Flutter + Rust) release,并实现「零环境交付」:
  把 VC++ 2015-2022 运行时 DLL 一起拷到发布目录(与 .exe 同级),最后压成 zip。

  干净的目标机没装 VC++ Redistributable 时,flutter_windows.dll 与 MSVC 编译的
  Rust *.dll 会因缺 msvcp140*/vcruntime140* 而双击闪退;本脚本把这些运行时一并打包,
  让成品双击即用、无需安装任何环境。

  用法(在 Win 机仓库根执行): powershell -ExecutionPolicy Bypass -File scripts\build_win.ps1
#>
#requires -Version 5
$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$desktop = Join-Path $repo 'desktop'
Write-Host "== repo:    $repo"
Write-Host "== desktop: $desktop"

# 1) 编译 release
Push-Location $desktop
try {
  flutter build windows --release
  if ($LASTEXITCODE -ne 0) { throw "flutter build windows failed ($LASTEXITCODE)" }
} finally {
  Pop-Location
}

$rel = Join-Path $desktop 'build\windows\x64\runner\Release'
if (-not (Test-Path $rel)) { throw "Release dir not found: $rel" }
Write-Host "== release: $rel"

# 2) 定位 VS 的 VC++ 可再分发 DLL 副本(优先官方 Redist,兜底 System32)
$crt = $null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) {
  $vs = & $vswhere -latest -products * -property installationPath
  if ($vs) {
    $redist = Join-Path $vs 'VC\Redist\MSVC'
    if (Test-Path $redist) {
      # 递归找 x64 下的 Microsoft.VC*.CRT(兼容不同版本目录命名;找不到不致命,回退 System32)
      $crtDir = Get-ChildItem $redist -Recurse -Directory -Filter 'Microsoft.VC*.CRT' -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match '\\x64\\' } |
                Sort-Object FullName -Descending | Select-Object -First 1
      if ($crtDir) { $crt = $crtDir.FullName }
    }
  }
}
Write-Host "== VC++ CRT source: $(if($crt){$crt}else{'(System32 fallback)'})"

$need = 'msvcp140.dll','msvcp140_1.dll','msvcp140_2.dll',
        'vcruntime140.dll','vcruntime140_1.dll','concrt140.dll','vccorlib140.dll'
foreach ($d in $need) {
  $src = $null
  if ($crt -and (Test-Path (Join-Path $crt $d)))      { $src = Join-Path $crt $d }
  elseif (Test-Path (Join-Path $env:WINDIR "System32\$d")) { $src = Join-Path $env:WINDIR "System32\$d" }
  if ($src) { Copy-Item $src $rel -Force; Write-Host "  [ok]   $d" }
  else      { Write-Host "  [MISS] $d" }
}

# 3) 压缩成交付 zip
$dist = Join-Path $repo 'dist'
New-Item -ItemType Directory -Force -Path $dist | Out-Null
$zip = Join-Path $dist 'Conch-windows-x64.zip'
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $rel '*') -DestinationPath $zip -CompressionLevel Optimal

$size = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host "ZIP_READY $zip ($size MB)"

# 4) 生成 NSIS 安装器(装完即用:桌面/开始菜单快捷方式 + 注册卸载项;Release 自带运行时)
$verLine = Select-String -Path (Join-Path $desktop 'pubspec.yaml') -Pattern '^version:\s*([0-9]+\.[0-9]+\.[0-9]+)'
$ver = if ($verLine) { $verLine.Matches[0].Groups[1].Value } else { '1.0.0' }

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source
if (-not $makensis) {
  foreach ($c in @((Join-Path $env:ProgramFiles 'NSIS\makensis.exe'),
                   (Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'))) {
    if (Test-Path $c) { $makensis = $c; break }
  }
}
if ($makensis) {
  $nsi   = Join-Path $repo 'packaging\windows\conch-setup.nsi'
  $icon  = Join-Path $repo 'desktop\windows\runner\resources\app_icon.ico'
  $setup = Join-Path $dist 'Conch-windows-x64-setup.exe'
  if (Test-Path $setup) { Remove-Item $setup -Force }
  & $makensis "/DSTAGE_DIR=$rel" "/DOUTPUT_EXE=$setup" "/DICON_PATH=$icon" "/DAPP_VERSION=$ver" $nsi
  if (Test-Path $setup) {
    $ssize = [math]::Round((Get-Item $setup).Length / 1MB, 1)
    Write-Host "SETUP_READY $setup ($ssize MB)"
  } else { Write-Host 'SETUP_FAILED (makensis 未产出安装器)' }
} else {
  Write-Host 'NSIS 未安装,跳过安装器(可先 scoop install nsis)'
}
