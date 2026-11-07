# Releasing cloud-sessions

Users install with **two commands and nothing else** (no Rust, no cargo, no Homebrew):

```
claude plugin marketplace add ramene/cloud-sessions
claude plugin install cloud-sessions@cloud-sessions
```

Reading and watching use Claude Code's built-in `RemoteTrigger` tool; sending uses the user's own `claude` command.
Only the optional extras (`/cloud-start`, `/cloud-allow`, `/cloud-list`) need the small `cloud-sessions` program, and the
plugin's launcher (`scripts/cloud-sessions`) fetches it by itself the first time one of them runs: from the release, into
the plugin's data folder, after checking the SHA-256 published next to it. If it cannot, it prints one plain sentence.

## What a release contains

| Asset (per version `X.Y.Z`) | Used by |
| --- | --- |
| `cloud-sessions-X.Y.Z-macos-universal.tar.gz` (+ `.sha256`) | the launcher on any Mac (arm64 and Intel) |
| `cloud-sessions-X.Y.Z-linux-x86_64.tar.gz` (+ `.sha256`) | the launcher on Linux x86_64 |
| `cloud-sessions-X.Y.Z-linux-aarch64.tar.gz` (+ `.sha256`) | the launcher on Linux arm64 |
| `SHA256SUMS` | people verifying by hand |

Each archive is `cloud-sessions-X.Y.Z-<label>/` holding `cloud-sessions`, `LICENSE`, `README.md` and a copy of the plugin.
The Linux binaries link only libc and libgcc (no libdbus, no OpenSSL): they need nothing installed. Build on the oldest
Linux you want to support, because glibc binaries run on newer glibc, not older.

## Build and checksum (no GitHub workflow needed)

```
cloud/release/build-all.sh            # builds what this machine can, writes cloud/release/out/SHA256SUMS
cloud/release/build-all.sh --bundle   # also copies the binaries into claude-mod/cloud-sessions/bin/<label>/
```

* On a Mac it builds `macos-universal` (arm64 + x86_64, merged with `lipo`). On Linux it builds its own CPU natively;
  with `cargo zigbuild` installed it cross-builds the other Linux target too.
* A target it cannot build is reported as SKIPPED and the script exits 2. Build the rest on another machine, copy those
  archives into `cloud/release/out/`, and run the script again: it re-checks versions and rewrites `SHA256SUMS` over
  everything there. Exit 0 means all three archives are present.
* `scripts/cloud-sessions/package.sh <label> <binary>` makes one archive and its `.sha256` (the build script calls it).

`.github/workflows` cannot be used (our token lacks workflow scope). A ready workflow is kept at
`cloud/inputs/github-workflows/cloud-sessions-release.yml` for whoever has the scope; it builds the same archives.

## Tagging 0.1.0 (described, NOT done)

1. `node scripts/cloud-sessions/check-release.mjs` (all manifests, `VERSION`, formula and `CHANGELOG.md` say 0.1.0), then
   `cargo test --workspace --locked`, `npm test` and `claude plugin validate .`.
2. `cloud/release/build-all.sh` on a Mac and on Linux (x86_64 and arm64) until it exits 0.
3. Publish the release **before** users install: create GitHub release `cloud-sessions-v0.1.0` and attach everything in
   `cloud/release/out/` (the `.tar.gz`, `.sha256` and `SHA256SUMS` files). Use the `## 0.1.0` section of `CHANGELOG.md` as notes.
4. Tag: `git tag cloud-sessions-v0.1.0 && git push origin cloud-sessions-v0.1.0` (not pushed; waits for the maintainer).
5. Check on a clean machine (no Rust): the two install commands, then `/cloud-allow` and `/cloud-start` once.

**Public assets are required.** The launcher downloads without credentials. While the repository is private an
unauthenticated download of a release asset fails, and users see "could not download … (offline, or the release is not
public yet)". Either publish the release assets where they can be fetched anonymously, or bundle the binaries in the plugin
(`--bundle`, then `git add -f claude-mod/cloud-sessions/bin`; about 4 MB per target; the launcher prefers them). The
marketplace itself also needs the repository to be reachable by the user.

## Licence

MIT, `Copyright (c) 2026 the cloud-sessions contributors`: a `LICENSE` in `claude-mod/cloud-sessions/`,
`crates/cloud-sessions-core/` and `crates/cloud-sessions-cli/`; every manifest says `MIT`.
