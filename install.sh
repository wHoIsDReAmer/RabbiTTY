#!/bin/sh
# Rabbitty installer for Linux and macOS.
#   curl -fsSL https://raw.githubusercontent.com/wHoIsDReAmer/RabbiTTY/main/install.sh | sh

set -e

REPO="wHoIsDReAmer/RabbiTTY"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/Applications"
XDG_DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
APPLICATIONS_DIR="${XDG_DATA_HOME}/applications"
ICON_DIR="${XDG_DATA_HOME}/icons/hicolor/256x256/apps"
DESKTOP_FILE="${APPLICATIONS_DIR}/io.github.rabbitty.desktop"
ICON_FILE="${ICON_DIR}/io.github.rabbitty.png"

err() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "${os}_${arch}" in
        Linux_x86_64)        echo "linux-amd64" ;;
        Linux_aarch64|Linux_arm64) echo "linux-arm64" ;;
        Darwin_arm64)        echo "macos-arm64" ;;
        *) echo "" ;;
    esac
}

latest_tag() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | sed -nE 's/.*"tag_name": *"([^"]+)".*/\1/p' \
        | head -n1
}

# The published archives are neither signed nor notarized, so the release
# SHA256SUMS file is the only integrity check available; treat it as fatal.
verify_sha256() {
    dir="$1"
    name="$2"

    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "${dir}/${name}" | cut -d' ' -f1)
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "${dir}/${name}" | cut -d' ' -f1)
    else
        err "no SHA-256 tool found (need sha256sum or shasum); refusing to install an unverified download."
    fi

    # Conventional "<hex>  <filename>" lines. The name is compared exactly
    # instead of via a regex so dots in the asset name cannot match loosely.
    expected=$(awk -v name="$name" \
        '{ f = $2; sub(/^\*/, "", f); if (f == name) { print $1; exit } }' \
        "${dir}/SHA256SUMS")
    [ -n "$expected" ] || err "no SHA256SUMS entry for ${name}; refusing to install an unverified download."

    actual=$(printf '%s' "$actual" | tr 'A-F' 'a-f')
    expected=$(printf '%s' "$expected" | tr 'A-F' 'a-f')

    if [ "$actual" != "$expected" ]; then
        err "checksum mismatch for ${name}
  expected: ${expected}
  actual:   ${actual}
The download is corrupt or has been tampered with; nothing was installed."
    fi
}

refresh_linux_desktop_databases() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
    fi

    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t "${XDG_DATA_HOME}/icons/hicolor" >/dev/null 2>&1 || true
    fi

    if command -v xdg-desktop-menu >/dev/null 2>&1; then
        xdg-desktop-menu forceupdate >/dev/null 2>&1 || true
    fi

    if command -v kbuildsycoca6 >/dev/null 2>&1; then
        kbuildsycoca6 >/dev/null 2>&1 || true
    elif command -v kbuildsycoca5 >/dev/null 2>&1; then
        kbuildsycoca5 >/dev/null 2>&1 || true
    fi
}

install_linux_desktop_entry() {
    bin_path="$1"
    icon_src=$(find "$tmp" -name 'logo.png' -type f -maxdepth 4 | head -n1)

    mkdir -p "$APPLICATIONS_DIR" "$ICON_DIR"

    if [ -n "$icon_src" ]; then
        install -m 0644 "$icon_src" "$ICON_FILE"
    else
        curl -fsSL \
            -o "$ICON_FILE" \
            "https://raw.githubusercontent.com/${REPO}/main/assets/logo.png" \
            >/dev/null 2>&1 \
            || printf 'Warning: failed to install application icon. The launcher may use a generic icon.\n' >&2
    fi

    cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Rabbitty
Comment=Fast, lean terminal emulator
Exec="$bin_path"
Icon=io.github.rabbitty
Terminal=false
Categories=System;TerminalEmulator;
StartupNotify=true
StartupWMClass=rabbitty
Keywords=terminal;shell;ssh;
EOF
    chmod 0644 "$DESKTOP_FILE"

    refresh_linux_desktop_databases
}

target=$(detect_target)
[ -n "$target" ] || err "unsupported OS/arch: $(uname -s) $(uname -m). Supported: Linux x86_64/aarch64, macOS arm64."

tag=$(latest_tag)
[ -n "$tag" ] || err "failed to resolve latest release tag from GitHub."

case "$target" in
    macos-*) ext="zip" ;;
    *)       ext="tar.gz" ;;
esac

asset="rabbitty-${tag}-${target}.${ext}"
url="https://github.com/${REPO}/releases/download/${tag}/${asset}"
sums_url="https://github.com/${REPO}/releases/download/${tag}/SHA256SUMS"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

printf 'Downloading %s...\n' "$asset"
curl -fsSL -o "${tmp}/${asset}" "$url" || err "download failed: $url"
curl -fsSL -o "${tmp}/SHA256SUMS" "$sums_url" \
    || err "failed to download release checksums: ${sums_url}"

printf 'Verifying checksum...\n'
verify_sha256 "$tmp" "$asset"

printf 'Extracting...\n'
case "$ext" in
    tar.gz) tar -xzf "${tmp}/${asset}" -C "$tmp" ;;
    zip)    unzip -q "${tmp}/${asset}" -d "$tmp" ;;
esac

case "$target" in
    macos-*)
        app_src=$(find "$tmp" -name 'Rabbitty.app' -type d -maxdepth 4 | head -n1)
        [ -n "$app_src" ] || err "Rabbitty.app not found in archive."
        mkdir -p "$APP_DIR"
        rm -rf "${APP_DIR}/Rabbitty.app"
        cp -R "$app_src" "$APP_DIR/"
        mkdir -p "$BIN_DIR"
        ln -sf "${APP_DIR}/Rabbitty.app/Contents/MacOS/rabbitty" "${BIN_DIR}/rabbitty"
        printf '\nInstalled Rabbitty.app to %s\n' "$APP_DIR"
        printf 'CLI symlink at %s/rabbitty\n' "$BIN_DIR"
        printf '\nThis build is unsigned and not notarized, so macOS blocks the\n'
        printf 'first launch. Open it deliberately: right-click Rabbitty.app in\n'
        printf 'Finder and choose Open, or clear the quarantine flag yourself:\n'
        printf '  xattr -d -r com.apple.quarantine "%s/Rabbitty.app"\n' "$APP_DIR"
        ;;
    linux-*)
        bin_src=$(find "$tmp" -name 'rabbitty' -type f -maxdepth 4 | head -n1)
        [ -n "$bin_src" ] || err "rabbitty binary not found in archive."
        mkdir -p "$BIN_DIR"
        install -m 0755 "$bin_src" "${BIN_DIR}/rabbitty"
        install_linux_desktop_entry "${BIN_DIR}/rabbitty"
        printf '\nInstalled rabbitty to %s/rabbitty\n' "$BIN_DIR"
        printf 'Desktop launcher at %s\n' "$DESKTOP_FILE"
        ;;
esac

case ":${PATH}:" in
    *:"${BIN_DIR}":*) ;;
    *)
        printf '\nWarning: %s is not in your PATH.\n' "$BIN_DIR"
        printf 'Add this to your shell profile (~/.bashrc, ~/.zshrc, ~/.profile):\n'
        printf '  export PATH="$HOME/.local/bin:$PATH"\n'
        ;;
esac

printf "\nDone. Run 'rabbitty' to start.\n"
