#!/usr/bin/env pwsh
#Requires -Version 5.1

<#
.SYNOPSIS
    Bifrost installation script for Windows
.DESCRIPTION
    Downloads and installs the Bifrost proxy server binary
.PARAMETER Version
    Specific version to install (e.g., v0.1.0). If not specified, installs the latest version.
.PARAMETER InstallDir
    Installation directory. Defaults to $env:LOCALAPPDATA\bifrost\bin
.EXAMPLE
    irm https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.ps1 | iex
.EXAMPLE
    .\install-binary.ps1 -Version v0.0.9-alpha
.EXAMPLE
    .\install-binary.ps1 -InstallDir "C:\Tools\bifrost"
#>

param(
    [string]$Version = "",
    [string]$InstallDir = ""
)

$ErrorActionPreference = "Stop"

$REPO = "bifrost-proxy/bifrost"
$BINARY_NAME = "bifrost"

if (-not $InstallDir) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "bifrost\bin"
}

function Write-Banner {
    Write-Host ""
    Write-Host "╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║                                                           ║" -ForegroundColor Cyan
    Write-Host "║   ____  _  __                _                            ║" -ForegroundColor Cyan
    Write-Host "║  |  _ \(_)/ _|_ __ ___  ___| |_                           ║" -ForegroundColor Cyan
    Write-Host "║  | |_) | | |_| '__/ _ \/ __| __|                          ║" -ForegroundColor Cyan
    Write-Host "║  |  _ <| |  _| | | (_) \__ \ |_                           ║" -ForegroundColor Cyan
    Write-Host "║  |_| \_\_|_| |_|  \___/|___/\__|                          ║" -ForegroundColor Cyan
    Write-Host "║                                                           ║" -ForegroundColor Cyan
    Write-Host "║   High-performance HTTP/HTTPS/SOCKS5 Proxy Server         ║" -ForegroundColor Cyan
    Write-Host "║                                                           ║" -ForegroundColor Cyan
    Write-Host "╚═══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Step {
    param([string]$Message)
    Write-Host "==> " -ForegroundColor Blue -NoNewline
    Write-Host $Message
}

function Write-Success {
    param([string]$Message)
    Write-Host "[OK] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[!] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Error {
    param([string]$Message)
    Write-Host "[X] " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Get-Architecture {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64" { return "x86_64" }
        "Arm64" { return "aarch64" }
        default { return "unknown" }
    }
}

function Get-Target {
    param([string]$Arch)
    switch ($Arch) {
        "x86_64" { return "x86_64-pc-windows-msvc" }
        "aarch64" { return "aarch64-pc-windows-msvc" }
        default { return $null }
    }
}

function Get-LatestVersion {
    $allReleasesUrl = "https://api.github.com/repos/$REPO/releases?per_page=10"
    
    try {
        $releases = Invoke-RestMethod -Uri $allReleasesUrl -UseBasicParsing -ErrorAction Stop
    }
    catch {
        if ($_.Exception.Message -match "rate limit") {
            Write-Error "GitHub API rate limit exceeded"
            Write-Warning "Please try again later or specify a version manually:"
            Write-Host "  .\install-binary.ps1 -Version v0.0.9-alpha"
            exit 1
        }
        Write-Error "Failed to fetch releases: $_"
        exit 1
    }

    if (-not $releases -or $releases.Count -eq 0) {
        Write-Error "No releases found for $REPO"
        Write-Warning "The project may not have published any releases yet."
        Write-Host ""
        Write-Host "You can build from source instead:"
        Write-Host "  git clone https://github.com/$REPO.git"
        Write-Host "  cd bifrost && cargo build --release"
        exit 1
    }

    $stableRelease = $releases | Where-Object { -not $_.prerelease } | Select-Object -First 1
    if ($stableRelease) {
        return $stableRelease.tag_name
    }

    Write-Warning "No stable release found, checking for pre-releases..."
    return $releases[0].tag_name
}

function Get-FileHash256 {
    param([string]$FilePath)
    $hash = Get-FileHash -Path $FilePath -Algorithm SHA256
    return $hash.Hash.ToLower()
}

function Install-Bifrost {
    Write-Banner

    $arch = Get-Architecture
    Write-Step "Detecting system..."
    Write-Host "  OS:           Windows"
    Write-Host "  Architecture: $arch"

    if ($arch -eq "unknown") {
        Write-Error "Unsupported architecture"
        exit 1
    }

    $target = Get-Target -Arch $arch
    if (-not $target) {
        Write-Error "No pre-built binary available for Windows-$arch"
        Write-Warning "You can build from source instead:"
        Write-Host "  git clone https://github.com/$REPO.git"
        Write-Host "  cd bifrost && cargo build --release"
        exit 1
    }

    if (-not $Version) {
        Write-Step "Fetching latest version..."
        $Version = Get-LatestVersion
    }

    Write-Success "Installing version: $Version"
    Write-Host "  Target: $target"

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    $tmpDir = Join-Path $env:TEMP "bifrost-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        Write-Step "Installing CLI..."

        $archiveFile = "bifrost-$Version-$target.zip"
        $archiveUrl = "https://github.com/$REPO/releases/download/$Version/$archiveFile"
        $checksumsUrl = "https://github.com/$REPO/releases/download/$Version/bifrost-$Version-checksums.txt"

        $archivePath = Join-Path $tmpDir $archiveFile
        $checksumsPath = Join-Path $tmpDir "checksums.txt"

        Write-Step "Downloading from: $archiveUrl"
        try {
            Invoke-WebRequest -Uri $archiveUrl -OutFile $archivePath -UseBasicParsing
        }
        catch {
            Write-Error "Failed to download binary: $_"
            exit 1
        }

        Write-Step "Downloading checksums..."
        try {
            Invoke-WebRequest -Uri $checksumsUrl -OutFile $checksumsPath -UseBasicParsing
        }
        catch {
            Write-Warning "Failed to download checksums, skipping verification"
            $checksumsPath = $null
        }

        if ($checksumsPath -and (Test-Path $checksumsPath)) {
            $checksumContent = Get-Content $checksumsPath
            $expectedChecksum = ($checksumContent | Where-Object { $_ -match $archiveFile } | ForEach-Object { ($_ -split '\s+')[0] })
            
            if ($expectedChecksum) {
                $actualChecksum = Get-FileHash256 -FilePath $archivePath
                if ($actualChecksum -ne $expectedChecksum.ToLower()) {
                    Write-Error "Checksum verification failed!"
                    Write-Error "Expected: $expectedChecksum"
                    Write-Error "Actual:   $actualChecksum"
                    exit 1
                }
                Write-Success "Checksum verified"
            }
            else {
                Write-Warning "Checksum not found for $archiveFile, skipping verification"
            }
        }

        Write-Step "Extracting..."
        $extractDir = Join-Path $tmpDir "extracted"
        Expand-Archive -Path $archivePath -DestinationPath $extractDir -Force

        $binaryName = "$BINARY_NAME.exe"
        $extractedDir = "bifrost-$Version-$target"
        $sourcePath = Join-Path $extractDir $extractedDir $binaryName

        if (-not (Test-Path $sourcePath)) {
            $sourcePath = Join-Path $extractDir $binaryName
        }

        if (-not (Test-Path $sourcePath)) {
            $foundBinary = Get-ChildItem -Path $extractDir -Filter $binaryName -Recurse | Select-Object -First 1
            if ($foundBinary) {
                $sourcePath = $foundBinary.FullName
            }
            else {
                Write-Error "Binary not found in archive"
                exit 1
            }
        }

        $destPath = Join-Path $InstallDir $binaryName
        Copy-Item -Path $sourcePath -Destination $destPath -Force

        Write-Success "CLI installed: $destPath"

        Write-Host ""
        Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        Write-Success "Installation completed!"
        Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        Write-Host ""

        $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($currentPath -notlike "*$InstallDir*") {
            Write-Warning "$InstallDir is not in your PATH"
            Write-Host ""
            Write-Host "To add it permanently, run:"
            Write-Host ""
            Write-Host "  `$currentPath = [Environment]::GetEnvironmentVariable('Path', 'User')" -ForegroundColor Gray
            Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"`$currentPath;$InstallDir`", 'User')" -ForegroundColor Gray
            Write-Host ""
            Write-Host "Or add it temporarily for this session:"
            Write-Host ""
            Write-Host "  `$env:Path += `";$InstallDir`"" -ForegroundColor Gray
        }

        Write-Host ""
        Write-Host "Getting started:"
        Write-Host ""
        Write-Host "  # Start proxy server"
        Write-Host "  bifrost start"
        Write-Host ""
        Write-Host "  # Start with custom port"
        Write-Host "  bifrost -p 8080 start"
        Write-Host ""
        Write-Host "  # Show help"
        Write-Host "  bifrost --help"
        Write-Host ""
        Write-Host "Documentation: https://github.com/$REPO"
        Write-Host ""
    }
    finally {
        if (Test-Path $tmpDir) {
            Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Install-Bifrost
