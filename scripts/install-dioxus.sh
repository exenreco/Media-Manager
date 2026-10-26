#!/usr/bin/env bash
set -euo pipefail
version=0.7.10
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) triple=x86_64-unknown-linux-gnu ;;
  Darwin-arm64) triple=aarch64-apple-darwin ;;
  *) echo 'Unsupported Dioxus CLI installer host' >&2; exit 1 ;;
esac
folder="${RUNNER_TEMP:?}/dioxus-cli"
mkdir -p "$folder"
archive="dx-${triple}.tar.gz"
base="https://github.com/DioxusLabs/dioxus/releases/download/v${version}"
curl --fail --location --retry 3 "$base/$archive" -o "$folder/$archive"
curl --fail --location --retry 3 "$base/dx-${triple}.sha256" -o "$folder/checksums"
expected=$(awk -v name="$archive" '$2 == name || $2 == "*" name {print $1}' "$folder/checksums")
test -n "$expected"
actual=$(shasum -a 256 "$folder/$archive" | awk '{print $1}')
test "$actual" = "$expected"
tar -xzf "$folder/$archive" -C "$folder"
binary=$(find "$folder" -type f -name dx | head -n 1)
test -n "$binary"
chmod +x "$binary"
dirname "$binary" >> "${GITHUB_PATH:?}"
