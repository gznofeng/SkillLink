# SkillLink 打包脚本：release 构建 + 组装 zip 到 release/ 目录（按版本号归档）
# 用法：powershell -File scripts\package.ps1            （版本号自动读自 app/Cargo.toml）
#       powershell -File scripts\package.ps1 -Version 0.2.0
param([string]$Version)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

# 1. 版本号：未显式传入时从 app/Cargo.toml 读取
if (-not $Version) {
    $cargo = Get-Content "$root\app\Cargo.toml" -Raw
    if ($cargo -match '(?m)^\s*version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        throw "无法从 app/Cargo.toml 读取版本号，请用 -Version 指定"
    }
}
Write-Host "==> 版本: v$Version"

# 2. release 构建
Write-Host "==> cargo build --release ..."
Push-Location $root
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release 失败" }
} finally {
    Pop-Location
}

# 3. 组装 staging 目录
$stage = "$root\package\SkillLink-v$Version"
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item "$root\target\release\skill-link.exe" $stage -Force
Copy-Item "$root\package\使用说明.txt" $stage -Force

# 4. 压缩到 release/（保留历史版本）
$releaseDir = "$root\release"
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
$zip = "$releaseDir\SkillLink-v$Version-win64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage -DestinationPath $zip

$sizeMB = [math]::Round((Get-Item $zip).Length / 1MB, 2)
Write-Host "==> 打包完成: $zip ($sizeMB MB)"
