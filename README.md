# Roves Packmaster

A desktop GUI for packaging a web-built game into [Roves](https://github.com/DRincs-Productions/roves)
distributions — portable binaries and installers for Windows, macOS, and Linux — without
needing Rust or Python installed. Point it at your game's built output, choose what to
generate, and it produces the releases for you.

Built with [Tauri](https://tauri.app/) (Rust + a native webview) so Packmaster itself
ships as a single, ordinary desktop app: the *user running Packmaster* never needs a Rust
or Python toolchain, even though *building Roves itself* still does (see the engine's own
[README](../README.md)) — Packmaster is meant to work from pre-built Roves engine
binaries rather than compiling Servo on your machine.

## Status

This is an early, UI-first pass: every screen described below is real and functional, but
there's no real Roves engine wired up yet behind the "generate release" button — it's a
working mock of the intended flow (see this project's own `CLAUDE.md` for what's expected
of any change made here in the meantime). The release folder it creates and opens at the
end is real; what gets put in it isn't, yet.

## What it does

1. **Pick your build output.** Point Packmaster at the folder your bundler already
   produced (a Vite.js project's `dist/`, or the equivalent from any other bundler) —
   not your source code. Packmaster checks for an `index.html` there before continuing.
2. **Configure your release:**
   - **Portable** — a self-contained, double-click-to-run bundle per platform, no
     installer or admin rights needed.
   - **Installable packages** — a real installer (`.msi`/`.deb`/`.dmg`) per platform,
     only shown for platforms your current system can actually build (each format needs
     a host-specific tool — WiX on Windows, `dpkg-deb` on Linux, `hdiutil` on macOS).
   - **Plugins** — currently just Steam integration, opt-in.
   - **Compression** — packs your game's content into compressed archives instead of
     loose files, with the same tunables (level, max archive size, exclusions) as the
     engine's own `mach bundle --content-compress` flags, defaulting to what the engine
     itself defaults to.
3. **Generate.** Shows progress, then opens the folder the release was written to —
   always a `release/` folder next to wherever Packmaster itself is running from.

Every screen shows the Roves icon and wordmark, and is available in nine languages
(English, Italian, Chinese, Japanese, Korean, Spanish, French, Russian, German) — see
`src/i18n/`.

## Development

```bash
npm install
npm run tauri dev
```

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
