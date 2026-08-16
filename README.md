# Roves Packmaster

A desktop GUI for packaging a web-built game into [Roves](https://github.com/DRincs-Productions/roves)
distributions — portable binaries for Windows, macOS, and Linux — without needing Rust or
Python installed. Point it at your game's built output, choose what to generate, and it
produces the releases for you.

Built with [Tauri](https://tauri.app/) (Rust + a native webview) so Packmaster itself
ships as a single, ordinary desktop app: the *user running Packmaster* never needs a Rust
or Python toolchain. Real bundling never shells out to `mach`/Python either — it downloads
the engine's own prebuilt, versioned shell release (`roves_shell_<platform>.zip`, see the
engine repo's [README](../README.md)) instead of compiling one, and packs your content into
it by linking the engine's `roves-content-packer` crate directly as a Rust library (see
`src-tauri/src/bundle.rs`). Building *Packmaster itself* is the only place a Rust toolchain
is still needed — see "Development" below, or grab a build from this repo's own [rolling
test release](https://github.com/DRincs-Productions/roves-ui/releases/tag/test) instead.

## Status

Real, not a mock: "Generate release" actually downloads the targeted shell version
(`src/lib/shell-version.ts`'s `TARGET_SHELL_VERSION`), packs your content into it, and
writes a real, runnable release to disk. **Portable only for now** — no `.msi`/`.deb`/`.dmg`
installers, since those need native per-platform tooling (WiX, `dpkg-deb`, `hdiutil`) that
downloading a prebuilt shell doesn't solve; the installers/plugins screens from an earlier
mock pass are hidden until that's real (see this project's own `CLAUDE.md`, "Hide, don't
just disable"). Steam integration is the same story — it needs a Steam-enabled prebuilt
shell variant that doesn't exist yet, so that toggle is gone for now too, not silently
ignored.

## What it does

1. **Pick your build output.** Point Packmaster at the folder your bundler already
   produced (a Vite.js project's `dist/`, or the equivalent from any other bundler) —
   not your source code. Packmaster checks for an `index.html` there before continuing.
2. **Configure your release:**
   - **Portable** — a self-contained, double-click-to-run bundle per platform. Each
     platform shows a real, live check (a HEAD request against the targeted shell
     release's actual asset) for whether it's currently distributable, rather than
     assuming it always is.
   - **Compression** — packs your game's content into compressed archives instead of
     loose files, with the same tunables (level, max archive size, exclusions) as the
     engine's own `mach bundle --content-compress` flags, defaulting to what the engine
     itself defaults to.
3. **Generate.** Downloads the shell (cached per version/platform after the first run),
   packs your content into it, and shows real progress per platform. Opens the folder the
   release was written to when done — a `release/` folder next to wherever Packmaster
   itself is running from, containing one `<your-game-name>_<platform>.zip` per platform.

Every screen shows the Roves icon and wordmark, and is available in nine languages
(English, Italian, Chinese, Japanese, Korean, Spanish, French, Russian, German) — see
`src/i18n/`.

## Development

```bash
npm install
npm run tauri dev
```

## Testing a build without compiling it yourself

`.github/workflows/test.yml` builds Packmaster (portable output — `--no-bundle` on
Windows/Linux, `--bundles app` on macOS, mirroring this project's own portable-only scope)
on every push, and publishes it to a rolling, unversioned ["test"
release](https://github.com/DRincs-Productions/roves-ui/releases/tag/test) — its assets are
overwritten on every run, the same pattern the main engine repo's own `test.yml` uses. Grab
a build from there if you don't have (or don't want to set up) a local Rust/Tauri
toolchain.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
