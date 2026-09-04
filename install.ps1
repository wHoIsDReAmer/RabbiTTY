# Rabbitty installer for Windows.
#   irm https://raw.githubusercontent.com/wHoIsDReAmer/RabbiTTY/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 still defaults to TLS 1.0/1.1, which GitHub rejects
# with an opaque "underlying connection was closed" error.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = 'wHoIsDReAmer/RabbiTTY'
$InstallDir = Join-Path $env:LOCALAPPDATA 'Rabbitty'

function Get-LatestTag {
    $resp = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
    return $resp.tag_name
}

# The archives are unsigned, so the release SHA256SUMS file is the only
# integrity check available; a mismatch must abort before extraction.
function Get-ExpectedHash {
    param(
        [string] $SumsPath,
        [string] $AssetName
    )

    foreach ($line in @(Get-Content -Path $SumsPath)) {
        # Conventional sha256sum format: "<hex>  <filename>", '*' marks binary mode.
        $fields = $line.Trim() -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].TrimStart('*') -eq $AssetName) {
            return $fields[0].ToLowerInvariant()
        }
    }

    return $null
}

# A raw registry write preserves REG_EXPAND_SZ but, unlike
# [Environment]::SetEnvironmentVariable, does not tell running processes to
# reload their environment block.
function Publish-EnvironmentChange {
    if (-not ('Rabbitty.Native' -as [type])) {
        Add-Type -Namespace 'Rabbitty' -Name 'Native' -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr SendMessageTimeout(System.IntPtr hWnd, uint Msg, System.IntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out System.IntPtr lpdwResult);
'@
    }

    $unused = [System.IntPtr]::Zero
    # HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG, 1s timeout.
    [void][Rabbitty.Native]::SendMessageTimeout([System.IntPtr] 0xffff, 0x001A, [System.IntPtr]::Zero, 'Environment', 0x0002, 1000, [ref] $unused)
}

function Add-InstallDirToUserPath {
    param([string] $Directory)

    $key = Get-Item -Path 'HKCU:\Environment'
    if ($key.GetValueNames() -contains 'Path') {
        # DoNotExpandEnvironmentNames keeps %USERPROFILE%-style entries intact;
        # the expanded form would be written back as a literal REG_SZ.
        $rawPath = [string] $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $kind = $key.GetValueKind('Path')
    }
    else {
        $rawPath = ''
        $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
    }

    $wanted = $Directory.TrimEnd('\')
    foreach ($entry in ($rawPath -split ';')) {
        if (-not $entry) { continue }
        $trimmed = $entry.TrimEnd('\')
        if (($trimmed -eq $wanted) -or ([Environment]::ExpandEnvironmentVariables($trimmed) -eq $wanted)) {
            Write-Host "$Directory is already in the user PATH."
            return
        }
    }

    $newPath = if ($rawPath) { "$rawPath;$Directory" } else { $Directory }
    $type = if ($kind -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) { 'ExpandString' } else { 'String' }
    Set-ItemProperty -Path 'HKCU:\Environment' -Name 'Path' -Value $newPath -Type $type

    try { Publish-EnvironmentChange } catch { }

    Write-Host "Added $Directory to user PATH (restart terminal to pick it up)."
}

$tag = Get-LatestTag
if (-not $tag) {
    Write-Error 'failed to resolve latest release tag from GitHub.'
    exit 1
}

$asset = "rabbitty-$tag-windows-amd64.zip"
$url = "https://github.com/$Repo/releases/download/$tag/$asset"
$sumsUrl = "https://github.com/$Repo/releases/download/$tag/SHA256SUMS"

$tmp = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "rabbitty-install-$(Get-Random)") -Force
try {
    $zipPath = Join-Path $tmp.FullName $asset
    Write-Host "Downloading $asset..."
    Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

    $sumsPath = Join-Path $tmp.FullName 'SHA256SUMS'
    try {
        Invoke-WebRequest -Uri $sumsUrl -OutFile $sumsPath -UseBasicParsing
    }
    catch {
        throw "failed to download release checksums from ${sumsUrl}: $($_.Exception.Message)"
    }

    Write-Host 'Verifying checksum...'
    $expected = Get-ExpectedHash -SumsPath $sumsPath -AssetName $asset
    if (-not $expected) {
        throw "no SHA256SUMS entry for $asset; refusing to install an unverified download."
    }

    $actual = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "checksum mismatch for ${asset}: expected $expected, got $actual. The download is corrupt or has been tampered with; nothing was installed."
    }

    Write-Host 'Extracting...'
    Expand-Archive -Path $zipPath -DestinationPath $tmp.FullName -Force

    $exe = Get-ChildItem -Path $tmp.FullName -Recurse -Filter 'rabbitty.exe' | Select-Object -First 1
    if (-not $exe) {
        throw 'rabbitty.exe not found in archive.'
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path $exe.FullName -Destination (Join-Path $InstallDir 'rabbitty.exe') -Force

    Add-InstallDirToUserPath -Directory $InstallDir

    Write-Host ""
    Write-Host "Installed rabbitty.exe to $InstallDir"
    Write-Host "Run 'rabbitty' in a new terminal to start."
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
