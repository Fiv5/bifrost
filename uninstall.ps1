#!/usr/bin/env pwsh
#Requires -Version 5.1

<#
.SYNOPSIS
    Bifrost uninstallation script for Windows
.DESCRIPTION
    Removes the Bifrost proxy server binary and optionally cleans up data/config files
.PARAMETER InstallDir
    Installation directory. Defaults to $env:LOCALAPPDATA\bifrost\bin
.PARAMETER Purge
    Also remove configuration and data files
.PARAMETER Yes
    Skip confirmation prompts
.EXAMPLE
    .\uninstall.ps1
.EXAMPLE
    .\uninstall.ps1 -Purge
.EXAMPLE
    .\uninstall.ps1 -Yes -Purge
#>

param(
    [string]$InstallDir = "",
    [switch]$Purge,
    [Alias("y")]
    [switch]$Yes
)

$ErrorActionPreference = "Stop"

$BINARY_NAME = "bifrost"

if (-not $InstallDir) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "bifrost\bin"
}

$DEFAULT_DATA_DIR = Join-Path $env:LOCALAPPDATA "bifrost\data"
$DEFAULT_CONFIG_DIR = Join-Path $env:LOCALAPPDATA "bifrost\config"
$ALT_DATA_DIR = Join-Path $env:USERPROFILE ".bifrost"
$ALT_CONFIG_DIR = Join-Path $env:APPDATA "bifrost"

function Write-Banner {
    Write-Host ""
    Write-Host "╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║                                                           ║" -ForegroundColor Cyan
    Write-Host "║   Bifrost Uninstaller                                     ║" -ForegroundColor Cyan
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

function Write-Warn {
    param([string]$Message)
    Write-Host "[!] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Err {
    param([string]$Message)
    Write-Host "[X] " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Confirm-Action {
    param([string]$Prompt)
    
    if ($Yes) {
        return $true
    }
    
    $response = Read-Host "$Prompt [y/N]"
    return ($response -eq 'y' -or $response -eq 'Y' -or $response -eq 'yes' -or $response -eq 'Yes')
}

function Find-DataDirs {
    $dirs = @()
    
    if (Test-Path $DEFAULT_DATA_DIR) {
        $dirs += $DEFAULT_DATA_DIR
    }
    if (Test-Path $ALT_DATA_DIR) {
        $dirs += $ALT_DATA_DIR
    }
    
    return $dirs
}

function Find-ConfigDirs {
    $dirs = @()
    
    if (Test-Path $DEFAULT_CONFIG_DIR) {
        $dirs += $DEFAULT_CONFIG_DIR
    }
    if (Test-Path $ALT_CONFIG_DIR) {
        $dirs += $ALT_CONFIG_DIR
    }
    
    return $dirs
}

function Uninstall-Bifrost {
    Write-Banner

    $binaryPath = Join-Path $InstallDir "$BINARY_NAME.exe"

    Write-Step "Checking installation..."

    $foundBinary = Test-Path $binaryPath
    $dataDirs = Find-DataDirs
    $configDirs = Find-ConfigDirs

    if ($foundBinary) {
        Write-Host "  Binary:  $binaryPath"
    }

    foreach ($dir in $dataDirs) {
        Write-Host "  Data:    $dir"
    }

    foreach ($dir in $configDirs) {
        Write-Host "  Config:  $dir"
    }

    if (-not $foundBinary -and $dataDirs.Count -eq 0 -and $configDirs.Count -eq 0) {
        Write-Warn "Bifrost is not installed or already uninstalled"
        Write-Host ""
        Write-Host "Checked locations:"
        Write-Host "  Binary: $binaryPath"
        Write-Host "  Data:   $DEFAULT_DATA_DIR, $ALT_DATA_DIR"
        Write-Host "  Config: $DEFAULT_CONFIG_DIR, $ALT_CONFIG_DIR"
        return
    }

    Write-Host ""

    if ($Purge) {
        Write-Warn "This will remove the binary and ALL data/configuration files!"
    }

    if (-not (Confirm-Action "Do you want to proceed with uninstallation?")) {
        Write-Host "Uninstallation cancelled."
        return
    }

    Write-Host ""

    if ($foundBinary) {
        Write-Step "Removing binary..."
        Remove-Item -Path $binaryPath -Force
        Write-Success "Removed: $binaryPath"

        $parentDir = Split-Path $binaryPath -Parent
        $remainingFiles = Get-ChildItem -Path $parentDir -ErrorAction SilentlyContinue
        if ($null -eq $remainingFiles -or $remainingFiles.Count -eq 0) {
            Remove-Item -Path $parentDir -Force -ErrorAction SilentlyContinue
        }
    }

    if ($Purge) {
        foreach ($dir in $dataDirs) {
            Write-Step "Removing data directory..."
            Remove-Item -Path $dir -Recurse -Force
            Write-Success "Removed: $dir"
        }

        foreach ($dir in $configDirs) {
            Write-Step "Removing config directory..."
            Remove-Item -Path $dir -Recurse -Force
            Write-Success "Removed: $dir"
        }

        $bifrostRoot = Join-Path $env:LOCALAPPDATA "bifrost"
        if (Test-Path $bifrostRoot) {
            $remainingItems = Get-ChildItem -Path $bifrostRoot -ErrorAction SilentlyContinue
            if ($null -eq $remainingItems -or $remainingItems.Count -eq 0) {
                Remove-Item -Path $bifrostRoot -Force -ErrorAction SilentlyContinue
            }
        }
    }
    else {
        if ($dataDirs.Count -gt 0 -or $configDirs.Count -gt 0) {
            Write-Host ""
            Write-Warn "Data and config files were preserved."
            Write-Host "  To remove them, run: .\uninstall.ps1 -Purge"
            foreach ($dir in $dataDirs) {
                Write-Host "  Data:   $dir"
            }
            foreach ($dir in $configDirs) {
                Write-Host "  Config: $dir"
            }
        }
    }

    Write-Host ""
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    Write-Success "Uninstallation completed!"
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    Write-Host ""

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -like "*$InstallDir*") {
        Write-Warn "You may want to remove $InstallDir from your PATH"
        Write-Host ""
        Write-Host "Run this command to remove it:" -ForegroundColor Gray
        Write-Host ""
        Write-Host "  `$path = [Environment]::GetEnvironmentVariable('Path', 'User')" -ForegroundColor Gray
        Write-Host "  `$path = (`$path -split ';' | Where-Object { `$_ -ne '$InstallDir' }) -join ';'" -ForegroundColor Gray
        Write-Host "  [Environment]::SetEnvironmentVariable('Path', `$path, 'User')" -ForegroundColor Gray
    }

    Write-Host ""
}

Uninstall-Bifrost
