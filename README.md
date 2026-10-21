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
```

In a browser, `http://localhost:1420/#autoscan&open=/Users/wanpeninsula&select&review`
drives Space Lens through every state against the mock backend, and
`#module=cleanup&autoscan&review=projects` (or `&confirm`, `&clean`) does the same for Cleanup,
and `#module=clutter&autoscan&review=similar` for My Clutter.

**Full Disk Access:** without it macOS hides Mail, Safari, containers and more. During
`tauri dev` the app runs inside your terminal, so grant access to the terminal (or IDE).

## Safety

Every removal passes `crates/core/src/safety.rs`, which refuses system paths, home and
standard folders, and `~/Library/*` roots, including when reached through a symlink.
User files go to the Trash; only regenerable data (caches, build output) is deleted outright.
Cleanup only acts on item ids from its own last scan report, so the UI can never ask it to
delete an arbitrary path, and anything carrying a warning needs a second confirmation.
Scans never download iCloud placeholders and never touch access times.
