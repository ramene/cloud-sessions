#!/bin/sh
# Package a built cloud-sessions binary: cloud/release/out/cloud-sessions-<version>-<label>.tar.gz plus a .sha256.
# usage: [OUT_DIR=dir] scripts/cloud-sessions/package.sh <label> <path to cloud-sessions binary>
# The archive holds cloud-sessions, LICENSE, README.md and the plugin folder, so one download is a full install.
set -eu
label="$1"; binary="$2"
root="$(cd "$(dirname "$0")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/crates/cloud-sessions-cli/Cargo.toml" | head -1)"
name="cloud-sessions-$version-$label"
out="${OUT_DIR:-$root/cloud/release/out}"; mkdir -p "$out"
stage="$out/$name"
rm -rf "$stage"; mkdir -p "$stage"
cp "$binary" "$stage/cloud-sessions"; chmod 755 "$stage/cloud-sessions"
cp "$root/claude-mod/cloud-sessions/LICENSE" "$stage/LICENSE"
cp "$root/claude-mod/cloud-sessions/README.md" "$stage/README.md"
cp -R "$root/claude-mod/cloud-sessions" "$stage/plugin"
tar -C "$out" -czf "$out/$name.tar.gz" "$name"
( cd "$out" && if command -v sha256sum >/dev/null; then sha256sum "$name.tar.gz"; else shasum -a 256 "$name.tar.gz"; fi > "$name.tar.gz.sha256" )
rm -rf "$stage"
echo "$out/$name.tar.gz"
