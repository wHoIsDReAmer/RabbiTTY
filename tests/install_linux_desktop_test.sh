#!/bin/sh
set -eu

script="${1:-install.sh}"

assert_contains() {
    needle="$1"
    if ! grep -Fq "$needle" "$script"; then
        printf 'missing expected installer content: %s\n' "$needle" >&2
        exit 1
    fi
}

assert_contains 'DESKTOP_FILE="${APPLICATIONS_DIR}/io.github.rabbitty.desktop"'
assert_contains 'ICON_FILE="${ICON_DIR}/io.github.rabbitty.png"'
assert_contains '[Desktop Entry]'
assert_contains 'Type=Application'
assert_contains 'Name=Rabbitty'
assert_contains 'Exec='
assert_contains 'Icon=io.github.rabbitty'
assert_contains 'Categories=System;TerminalEmulator;'
assert_contains 'StartupWMClass=rabbitty'
assert_contains 'update-desktop-database'
assert_contains 'gtk-update-icon-cache'
assert_contains 'kbuildsycoca'

printf 'install_linux_desktop_test: ok\n'
