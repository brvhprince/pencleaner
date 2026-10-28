# PenCleaner

A macOS cleaner in the spirit of CleanMyMac: find what's eating your disk, clear junk,
and keep the machine quick. Rust engine, Tauri 2 shell, Svelte 5 UI.

## Layout

```
crates/core   scanning and cleanup engine (no Tauri dependency, unit-tested)
src-tauri     IPC commands, progress channels, app state
ui            Svelte 5 frontend (Vite root)
```

## Develop

```bash
pnpm install
pnpm tauri dev                 # the app
pnpm dev                       # UI only, in a browser, backed by ui/src/lib/mock.ts
cargo test -p pencleaner-core  # engine tests
cargo run --release -p pencleaner-core --example scan -- ~/Sites   # Space Lens from the terminal
cargo run --release -p pencleaner-core --example cleanup [developer] # Cleanup dry run (never deletes)
cargo run --release -p pencleaner-core --example clutter [folder]    # My Clutter dry run (never deletes)
cargo run --release -p pencleaner-core --example apps                # Applications dry run (never deletes)
cargo run --release -p pencleaner-core --example perf                # Performance readout (runs no tasks)
cargo run --release -p pencleaner-core --example updates             # App Updater check (installs nothing)
cargo run --profile scan -p pencleaner-core --example protect        # malware + background-item scan (removes nothing)
cargo run --profile scan -p pencleaner-core --example protect -- --privacy   # privacy traces + app permissions
cargo run --release -p pencleaner-core --example verify_update -- /Applications/Maccy.app
                                                                     # full download + verification, no install
```

In a browser, `http://localhost:1420/#module=spacelens&autoscan&open=/Users/wanpeninsula&select&review`
drives Space Lens through every state against the mock backend, and
`#module=cleanup&autoscan&review=projects` (or `&confirm`, `&clean`) does the same for Cleanup,
`#module=clutter&autoscan&review=similar` for My Clutter, and
`#module=applications&autoscan&review=apps&expand` (or `&confirm`) for Applications, and
`#module=performance&autoscan&review=background` (or `tasks&run`, `memory`, `snapshots`) for Performance,
`#module=smartcare&autoscan&run` for Smart Care, and `#module=activity` (or `&noactivity`) for My Activity.

## Modules

| Module | What it does |
|---|---|
| Smart Care | One scan across Cleanup, Performance and app leftovers; runs only safe, recommended items |
| Cleanup | System, developer, AI, Mail and Trash junk |
| Performance | Maintenance tasks, memory, login and background items, Time Machine snapshots |
| Protection | Malware scan with Apple's XProtect rules, suspicious background items, privacy traces, app permissions, background watcher |
| Applications | Uninstaller with support files, App Updater, leftovers of removed apps, installer files |
| My Clutter | Duplicates, similar images, large and old files, Downloads |
| Space Lens | Bubble map of what's using the disk |
| Settings | Thresholds, network switches, watcher, and the Ignore List (⌘,) |
| Menu bar | Mac health, protection status, disk/memory/CPU/battery/network/Trash tiles, one recommendation |
| Scheduled Smart Care | Remind, or clean self-rebuilding junk (caches, logs, temp files) on a schedule; never prompts |
| Open at Login | Registered with SMAppService; starts in the menu bar only |
| My Activity | History of everything freed, removed or run (`~/Library/Application Support/com.pen.cleaner/activity.jsonl`) |

## Settings and the Ignore List

Stored in `~/Library/Application Support/com.pen.cleaner/settings.json`. Ignore rules are either
a **folder** (never scanned or reported by any module) or a single **item** keyed
`<module>:<id>` (e.g. `updates:/Applications/X.app@2.0` skips one version only). Rules are
applied in Rust right after each scan and to cached results the moment you add one, so a
clean can never reach an ignored item. Malware detections can't be ignored.

## Release build

```bash
scripts/release.sh             # build, sign, verify, self-test, install to /Applications
scripts/release.sh --dmg       # also produce a .dmg
```

The build is signed with a stable identity whose SHA-1 lives in `.signing-identity`
(gitignored; list candidates with `security find-identity -v -p codesigning`). A free Apple
Development certificate from Xcode works for your own Mac. Stability matters: macOS ties Full
Disk Access to the signature, so ad-hoc builds would lose it on every rebuild. Sharing the app
with others needs a Developer ID certificate and notarization instead.

Signed builds run with the hardened runtime. `src-tauri/Entitlements.plist` grants
`allow-unsigned-executable-memory` because Protection's engine (yara-x → wasmtime) generates
code at runtime; without it the process is killed ("Code Signature Invalid"). The release
script's `--self-test` step catches that before installing.

**Full Disk Access:** without it macOS hides Mail, Safari, containers and more. During
`tauri dev` the app runs inside your terminal, so grant access to the terminal (or IDE).

## Safety

Every removal passes `crates/core/src/safety.rs`, which refuses system paths, home and
standard folders, and `~/Library/*` roots, including when reached through a symlink.
User files go to the Trash; only regenerable data (caches, build output) is deleted outright.
Anything that needs root (root-owned apps, system launch daemons) goes through one
standard macOS password prompt, and only for paths on a short allowlist
(`/Applications/*.app`, `/Library/{LaunchAgents,LaunchDaemons,PrivilegedHelperTools,…}/<item>`).
Cleanup only acts on item ids from its own last scan report, so the UI can never ask it to
delete an arbitrary path, and anything carrying a warning needs a second confirmation.
Scans never download iCloud placeholders and never touch access times.

## License

MIT. See [LICENSE](LICENSE). 
