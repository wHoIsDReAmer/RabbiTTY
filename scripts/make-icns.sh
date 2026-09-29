#!/bin/sh
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
src="$root/assets/logo.png"
out="$root/assets/icon.icns"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
set_dir="$work/icon.iconset"
mkdir -p "$set_dir"

for size in 16 32 128 256 512; do
    double=$((size * 2))
    sips -z "$size" "$size" "$src" --out "$set_dir/icon_${size}x${size}.png" >/dev/null
    sips -z "$double" "$double" "$src" --out "$set_dir/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil -c icns "$set_dir" -o "$out"
printf 'wrote %s\n' "$out"
