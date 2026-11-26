# Roves Packmaster

The desktop outputs are built from the engine's **CEF players** (engine v0.9.0 and later).
The frontend and Rust packager read the same `src/lib/shell-version.txt` pin; the displayed
version and downloaded release cannot drift through separate version constants. The pin is
v0.9.0, the first CEF player release (v0.7.15 and older are Servo shells with a different
layout).

A desktop GUI for packaging a web-built game into [Roves](https://github.com/DRincs-Productions/roves)
distributions — portable binaries for Windows, macOS, and Linux — without needing Rust or
Python installed. Point it at your game's built output, choose what to generate, and it
produces the releases for you.

Built with [Tauri](https://tauri.app/) (Rust + a native webview) so Packmaster itself
ships as a single, ordinary desktop app: the *user running Packmaster* never needs a Rust
or Python toolchain. Real bundling never shells out to `mach`/Python either — it downloads
the engine's own prebuilt, versioned player package (`roves_shell_<platform>.zip`, see the
engine repo's [README](../README.md)) instead of compiling one, and adds your game to it (see `src-tauri/src/bundle.rs` and `src-tauri/src/cef_player.rs`). Building *Packmaster itself* is the only place a Rust toolchain
is still needed — see "Development" below, or grab a build from this repo's own [rolling
test release](https://github.com/DRincs-Productions/roves-packmaster/releases/tag/test) instead.

## Status

Real, not a mock: "Generate release" actually downloads the targeted engine release's player
package (`src/lib/shell-version.ts`'s `TARGET_SHELL_VERSION`), puts your build in it, and
writes a real, runnable release to disk. Installers (`.msi`/`.dmg`/`.deb`) are real too
(`src-tauri/src/installer.rs`, porting the engine's own WiX/`hdiutil`/`dpkg-deb` logic to
Rust) — but unlike the portable path, each one only works when Packmaster itself is running
on its matching OS *and* that OS's native tool is already installed (WiX on Windows,
`hdiutil` on macOS — built in, `dpkg-deb` on Linux); Packmaster shows a real, live check for
this per platform rather than assuming. Steam is real too: toggling it on (and entering your
Steam App ID) downloads the engine's Steam player package (`roves_shell_<platform>_steam.zip`)
instead of the plain one, and writes a `steam_appid.txt` next to the packaged executable
(`Contents/MacOS/` inside the macOS app), with its own live availability check per platform,
same as the portable/installable paths above.

### Desktop packages (CEF players)

| Platform | Engine asset | Your game goes to | Window icon | Executable/bundle icon |
| --- | --- | --- | --- | --- |
| Windows | `roves_shell_windows[_steam\|_mcp].zip` | `game\` next to `play.exe` | `icon.png` next to `play.exe` | `play.exe` icon resource |
| macOS (arm64) | `roves_shell_macos[_steam\|_mcp].zip` | `<name>.app/Contents/Resources/game/` | `Contents/Resources/icon.png` | the `CFBundleIconFile` `.icns` |
| Linux (x64) | `roves_shell_linux[_steam\|_mcp].zip` | `game/` next to the executable | `icon.png` next to the executable | — (no `.desktop` entry) |

- The engine files are copied unchanged, entry by entry, so executable modes, the setuid
  `chrome-sandbox` mode and any symlink survive whichever OS Packmaster runs on. Extracting
  the Linux zip as a normal user still drops the setuid bit and root owner: on hosts that
  restrict unprivileged user namespaces players must re-apply
  `sudo chown root:root chrome-sandbox && sudo chmod 4755 chrome-sandbox` (the `.deb` installs
  it that way already).
- The Windows package is checked against its own `SHA256SUMS.txt` (every file, hidden ones
  included) and gets a regenerated one; macOS/Linux packages are checked for the files the
  engine documents (and the Steam library exactly in the Steam variant).
- **Project icon**: the Release info icon, else your build's `icon.png`, else the largest frame
  of its `icon.ico`/`favicon.ico`; with none, the Roves icon is used (`roves.ico` from the
  Windows package for `play.exe`). The `.ico`, the macOS `.icns` and the `play.exe` icon
  resource are all produced in-process, so Packmaster on any OS gives the same result. The
  sample game's icon is never carried over (the base `play.exe` carries it, so its main icon
  group is always replaced).
- **AI interaction (MCP, Advanced)**: off by default. When on, every desktop output is built
  from the engine's MCP-enabled development package (`roves_shell_<platform>_mcp.zip`) and
  named `<name>_<platform>_mcp.zip`; an AI agent or IDE connected over MCP can then read the
  running game's state and console, run JavaScript in the page, click, press keys and type
  (needs Node.js 20+, see `mcp/README.md` in the package). The `mcp/` folder sits next to
  `game/`: next to `play.exe` on Windows, in `<name>.app/Contents/Resources/` on macOS, next
  to the executable on Linux. Never ship these packages to players: any program running as
  your user account can control the game through them. It can't be combined with Steam (no
  such engine package). When off, the package contains no `mcp/` folder.

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
     generated files and packages.
   - **Portable** — a self-contained, double-click-to-run bundle per platform. Each
     platform shows a real, live check (a HEAD request against the targeted shell
     release's actual asset) for whether it's currently distributable, rather than
     assuming it always is.
   - **Installable** — a real `.msi`/`.dmg`/`.deb` per platform, each with its own live
     host-OS-and-tool availability check (see "Status" above).
   - **Advanced** — Steam (a toggle plus your Steam App ID — downloads the Steam player
     package and writes a `steam_appid.txt` next to the packaged executable; see "Status"
     above) and AI interaction (the players' local MCP bridge, see "Desktop
     packages").
   - **Mobile (Android, experimental)** — builds a real, installable `.apk` alongside your
     desktop release, downloading a JRE and the Android SDK on first use (no NDK needed, no
     native/Rust compilation involved — the game runs on the system's own Android WebView) —
     no Android Studio needed either. Debug-signed by default; the same card can also
     generate or import a signing keystore to produce a real, release-signed `.apk`.
   - **Mobile (iOS, experimental)** — builds an unsigned iOS Simulator app, or, with an
     imported Apple Distribution certificate + provisioning profile, a real, signed `.ipa`.
     Only available when Packmaster itself is running on macOS with Xcode and
     [XcodeGen](https://github.com/yonaskolb/XcodeGen) installed — unlike Android, there's no
     way around this (Apple's own toolchain has no cross-platform equivalent). Unlike
     Android's keystore, the certificate/provisioning profile can't be generated locally —
     only imported, from your own Apple Developer Program account.
     See the
     [wiki](https://github.com/DRincs-Productions/roves-wiki/blob/main/content/docs/packmaster.mdx)
     for the current caveats (Windows Android support is new and unverified against a real
     device; iOS is untested end to end on a real macOS machine).
3. **Generate.** Downloads the player package (cached per version/asset after the first
   run), adds your build as its game, and shows real, per-step progress. Opens
   the folder the release was written to when done — a `release/` folder next to wherever
   Packmaster itself is running from, containing one `<your-game-name>_<platform>.zip` per
   platform (`<name>/` inside on Windows/Linux, the `<name>.app` itself on macOS).

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
release](https://github.com/DRincs-Productions/roves-packmaster/releases/tag/test) — its assets are
overwritten on every run, the same pattern the main engine repo's own `test.yml` uses. Grab
a build from there if you don't have (or don't want to set up) a local Rust/Tauri
toolchain.

## Real, versioned releases

Tagged `v<major>.<minor>.<patch>` releases (e.g.
[`v0.1.0`](https://github.com/DRincs-Productions/roves-packmaster/releases/tag/v0.1.0)) are built by
`.github/workflows/release.yml` and published to their own, stable [GitHub
Releases](https://github.com/DRincs-Productions/roves-packmaster/releases) page — unlike the rolling
"test" build above, these target a pinned engine shell version and never change once
published. Prefer one of these over the "test" build for anything other than trying out a
change that hasn't shipped yet.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
