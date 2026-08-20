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
writes a real, runnable release to disk. Installers (`.msi`/`.dmg`/`.deb`) are real too
(`src-tauri/src/installer.rs`, porting the engine's own WiX/`hdiutil`/`dpkg-deb` logic to
Rust) — but unlike the portable path, each one only works when Packmaster itself is running
on its matching OS *and* that OS's native tool is already installed (WiX on Windows,
`hdiutil` on macOS — built in, `dpkg-deb` on Linux); Packmaster shows a real, live check for
this per platform rather than assuming. Steam is real too: toggling it on (and entering your
Steam App ID) downloads the engine's Steam-enabled shell variant instead of the plain one,
and writes a `steam_appid.txt` next to the packaged executable, with its own live
availability check per platform, same as the portable/installable paths above.

## What it does

1. **Pick your build output.** Point Packmaster at the folder your bundler already
   produced (a Vite.js project's `dist/`, or the equivalent from any other bundler) —
   not your source code. Packmaster checks for an `index.html` there, and rejects a
   folder that still has its own `package.json` (a project's source root, not its built
   output — Vite ships an `index.html` at the source root too, so that check alone can't
   tell the two apart).
2. **Configure your release:**
   - **Release info** — game name and version, read from the build folder's parent
     `package.json` when present (the version always follows it fresh; the name is
     remembered per source folder otherwise) and editable either way. These name the
     generated files and the bundled window title.
   - **Portable** — a self-contained, double-click-to-run bundle per platform. Each
     platform shows a real, live check (a HEAD request against the targeted shell
     release's actual asset) for whether it's currently distributable, rather than
     assuming it always is.
   - **Installable** — a real `.msi`/`.dmg`/`.deb` per platform, each with its own live
     host-OS-and-tool availability check (see "Status" above).
   - **Advanced** — Compression (packs your game's content into compressed archives
     instead of loose files, with the same tunables as the engine's own `mach bundle
     --content-compress` flags) and Steam (a toggle plus your Steam App ID — downloads the
     Steam-enabled shell variant and writes a `steam_appid.txt` next to the packaged
     executable; see "Status" above).
3. **Generate.** Downloads the shell (cached per version/platform after the first run),
   packs your content into it, and shows real, per-step progress. Opens the folder the
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

## Real, versioned releases

Tagged `v<major>.<minor>.<patch>` releases (e.g.
[`v0.1.0`](https://github.com/DRincs-Productions/roves-ui/releases/tag/v0.1.0)) are built by
`.github/workflows/release.yml` and published to their own, stable [GitHub
Releases](https://github.com/DRincs-Productions/roves-ui/releases) page — unlike the rolling
"test" build above, these target a pinned engine shell version and never change once
published. Prefer one of these over the "test" build for anything other than trying out a
change that hasn't shipped yet.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
