#!/bin/sh
# Build the cloud-sessions release archives for every target this machine can build, then write checksums.
#
#   cloud/release/build-all.sh [--bundle]
#
# Targets (labels the plugin's launcher downloads): macos-universal (arm64 + x86_64 merged with lipo),
# linux-x86_64, linux-aarch64. macOS builds the first; Linux builds its own CPU natively. A target this machine
# cannot build is reported as SKIPPED, not failed: build it elsewhere and copy its archive into cloud/release/out/, then run this
# script again (it re-writes cloud/release/out/SHA256SUMS over everything there). Cross-building Linux needs `cargo zigbuild`,
# used automatically when installed.
# The script exits 0 only when all three archives are in cloud/release/out/.
#   --bundle  also copy each built binary to claude-mod/cloud-sessions/bin/<label>/ so the plugin carries it
#             (about 4 MB each); the launcher prefers a bundled binary over downloading.
# Each archive is cloud/release/out/cloud-sessions-<version>-<label>.tar.gz plus a .sha256 file (the one the launcher verifies).
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
bundle=0; [ "${1:-}" = "--bundle" ] && bundle=1
version="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/cloud-sessions-cli/Cargo.toml | head -1)"
node scripts/cloud-sessions/check-release.mjs "$version"
out=cloud/release/out
mkdir -p "$out"
built=""; skipped=""

keep() { # label, binary
  scripts/cloud-sessions/package.sh "$1" "$2" >/dev/null
  built="$built $1"
  if [ "$bundle" = 1 ]; then mkdir -p "claude-mod/cloud-sessions/bin/$1"; cp "$2" "claude-mod/cloud-sessions/bin/$1/cloud-sessions"; fi
  echo "built $1"
}
build() { # rust target
  if [ -n "${2:-}" ] && command -v cargo-zigbuild >/dev/null 2>&1 && [ "$(host)" != "$1" ]; then
    cargo zigbuild --release --locked -p cloud-sessions-cli --target "$1.${LINUX_GLIBC:-2.28}"
  else
    cargo build --release --locked -p cloud-sessions-cli --target "$1"
  fi
}
host() { rustc -vV | sed -n 's/^host: //p'; }

case "$(uname -s)" in
  Darwin)
    rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null 2>&1 || true
    build aarch64-apple-darwin; build x86_64-apple-darwin
    mkdir -p target/universal
    lipo -create -output target/universal/cloud-sessions target/aarch64-apple-darwin/release/cloud-sessions target/x86_64-apple-darwin/release/cloud-sessions
    lipo -info target/universal/cloud-sessions
    keep macos-universal target/universal/cloud-sessions
    ;;
  *) skipped="$skipped macos-universal(needs a Mac)" ;;
esac
for pair in x86_64-unknown-linux-gnu:linux-x86_64 aarch64-unknown-linux-gnu:linux-aarch64; do
  triple="${pair%%:*}"; label="${pair##*:}"
  if [ "$(uname -s)" = Linux ] && [ "$(host)" = "$triple" ]; then
    build "$triple"; keep "$label" "target/$triple/release/cloud-sessions"
  elif command -v cargo-zigbuild >/dev/null 2>&1; then
    rustup target add "$triple" >/dev/null 2>&1 || true
    build "$triple" zig; keep "$label" "target/$triple/release/cloud-sessions"
  else
    skipped="$skipped $label"
  fi
done

( cd "$out" && rm -f SHA256SUMS && for f in cloud-sessions-"$version"-*.tar.gz; do [ -f "$f" ] && cat "$f.sha256"; done > SHA256SUMS; cat SHA256SUMS )
echo "built:$built"
[ -z "$skipped" ] || echo "SKIPPED:$skipped"
missing=""
for label in macos-universal linux-x86_64 linux-aarch64; do [ -f "$out/cloud-sessions-$version-$label.tar.gz" ] || missing="$missing $label"; done
[ -z "$missing" ] && { echo "all targets present: ready to tag cloud-sessions-v$version (see cloud/release/RELEASE.md)"; exit 0; }
echo "MISSING archives:$missing (build them on the right machine, copy into cloud/release/out/, run this script again)"; exit 2
