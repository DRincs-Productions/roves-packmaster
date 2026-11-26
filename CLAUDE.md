# Roves Packmaster (roves-packmaster)

A Vite + React + Tauri desktop app that gives game developers a GUI for packaging their
built web content into Roves distributions (portable binaries, `.msi`/`.deb`/`.dmg`
installers) — without needing Rust or Python installed themselves. See `README.md` for
what it actually does today and what it doesn't yet.

Routing: [TanStack Router](https://tanstack.com/router), file-based (`src/routes/`).
Components: [shadcn](https://ui.shadcn.com/) (Base UI style, see `components.json`).
i18n: [i18next](https://www.i18next.com/)/`react-i18next` (`src/i18n/`).
Formatting/linting: [Biome](https://biomejs.dev/) — **not** Oxlint or ESLint.

## Standing authorization: autonomous CI/push, and GitHub PAT usage

The user has given standing permission to push and run/watch CI
autonomously across this whole ecosystem (this repo, `../roves`, `../roves-action`, and the
rest) without stopping to ask first — see `../roves/CLAUDE.md`'s "Standing authorization:
autonomous CI/push/release loop" section for the full detail (report honestly if asked
directly whether something's done; don't claim it prematurely, and this doesn't cover
genuinely destructive actions outside the normal build/test/release loop).

When the user shares a GitHub PAT for reading Actions logs/annotations: it's **read-only** —
a write call like re-running failed jobs will 403 by design, use a real `git push` (or an
empty commit, `git commit --allow-empty`, when retrying after a transient flake with no real
code change) instead. Never persist it to a file or commit; re-export it fresh in every
command that needs it (env vars don't survive across separate tool calls here); if it starts
401ing, say so and ask for a fresh one. Full detail in `../roves/CLAUDE.md`.

## CRITICAL: format and lint before finishing any change

**Every time you change a file in this project, before considering the change done:**

```bash
npm run check   # biome check --write . -- formats AND lints, applying safe fixes
```

Read what it reports even after auto-fixing — some findings (accessibility, suspicious
logic) need a real code change, not just reformatting. Don't hand-format code to match
Biome's style guesses; run the tool.

## CRITICAL: check shadcn for a component before hand-building one

**Before writing a UI primitive from scratch — an accordion, a dialog, a dropdown, a date
picker, anything that isn't specific to Packmaster's own screens — check whether
[shadcn's registry](https://ui.shadcn.com/docs/components) already has one:**

```bash
npx shadcn@latest add <component>
```

This is how every component under `src/components/ui/` got here — they're vendored,
generated files, not hand-authored. If shadcn has a matching component:

- Add it via the CLI (as above) rather than writing the markup/behavior yourself.
- If you're not the one who can run it right now, say so explicitly and ask for it to be
  added, rather than silently hand-rolling a substitute.

Only write a component from scratch when shadcn's registry genuinely has nothing that
fits (e.g. Packmaster's own brand header, or a screen-specific layout).

## CRITICAL: every user-facing string needs a translation, in every supported language

Packmaster ships in nine languages (see `src/i18n/index.ts`'s `supportedLanguages`):
English, Italian, Chinese, Japanese, Korean, Spanish, French, Russian, German.

**No hardcoded user-facing string is allowed outside `src/i18n/locales/*.json`.** Adding
or changing any text a user sees means:

1. Add the key to `src/i18n/locales/en.json` (the reference locale).
2. Add the matching, actually-translated key to **all eight** other locale files in the
   same turn — not just English with the rest left to "do later." A missing key falls
   back to English at runtime (see `i18n/index.ts`'s `fallbackLng`), which silently hides
   the gap instead of erroring, so nothing will visibly break if you skip this — check
   anyway.
3. Reference it via `useTranslation()`'s `t()` in the component, never inline text.

## Settings persistence

User-adjustable settings (see `src/lib/settings.ts`) are persisted via
`@tauri-apps/plugin-store` and reloaded on every launch — a setting a user changes once
should still be set the next time they open Packmaster. Any new setting needs:

- A field in `PackmasterSettings` (`src/lib/settings.ts`) with a sensible default —
  mirroring the underlying engine's own `mach bundle` default where one exists (see that
  file's own comment on this).
- A merge entry in `loadSettings`'s defaults-merge (so an older stored settings file
  missing the new field doesn't break).
- See also the main engine repo's own `CLAUDE.md` (sibling checkout, `../CLAUDE.md`) —
  it requires asking, whenever a new `mach bundle` setting is added there, whether it
  should get a home here too. This is the other half of that same obligation: a setting
  added *here* should have a real, working default matching what the engine itself does.

## Platform-conditional UI

Some settings are only meaningful on a specific host OS: `.msi` needs Windows+WiX, `.dmg`
needs macOS+`hdiutil`, `.deb` needs Linux+`dpkg-deb` (see "Real bundling backend" below).
Rather than a frontend-side OS guess, this is backed by a real, live check —
`check_installer_availability` in `src-tauri/src/installer.rs`, surfaced to the frontend via
`src/lib/installer-availability.ts` — since availability depends on the native tool actually
being installed, not just which OS Packmaster is running on. `configure.tsx`'s installer
cards disable themselves (not hide — the platform stays visible, with the reason shown)
when that check comes back negative, so a user can still see what's possible elsewhere
without being able to try an option that can't work here.

## Real bundling backend

"Generate release" is real, not a mock — see `src-tauri/src/{shell,roves_cli,bundle}.rs`:

- **roves-cli sidecar (desktop packaging):** the packaging is `@drincs/roves-cli`, the same
  code roves-action runs, shipped as a standalone executable (Node.js Single Executable
  Application, built by roves-cli's `npm run build:sea` and attached to every roves-cli GitHub
  Release as `roves-<target-triple>[.exe]`). Tauri bundles it as `bundle.externalBin`
  (`binaries/roves`, i.e. `src-tauri/binaries/roves-<target-triple>[.exe]`, git-ignored) and puts
  it next to Packmaster's executable as `roves[.exe]`, where `roves_cli::sidecar_path` finds it
  (also in `tauri dev`). `npm run sidecar` (`scripts/fetch-roves-cli.mjs`) downloads it; both
  CI workflows run it before `tauri build`, and stage `roves[.exe]` next to the portable
  Windows/Linux executable (the macOS `.app` carries it in `Contents/MacOS`).
- **One roves-cli pin:** `src/lib/roves-cli-version.txt` (the exact CLI version, no `v`). To
  bump it: release roves-cli (its Release workflow publishes the npm package and the
  standalone executables), check `npm run sidecar` downloads them, then update the file. A
  CLI change of `bundle`'s options or `--json`/`--progress json` output needs a matching
  change in `roves_cli.rs` (argument builder, result and progress parsing; unit tests there).
- **One shell pin:** update `src/lib/shell-version.txt` after verifying a new engine release.
  Vite imports it as raw text for the frontend `TARGET_SHELL_VERSION`; Rust embeds the same
  file with `include_str!`. Both trim whitespace. Never reintroduce a separate version literal:
  the former Rust constant remained at v0.4.18 while the frontend reached v0.7.12.
- **`shell.rs`** decides which engine release the sidecar targets (`--shell-version`), checks
  that release really publishes an asset (the live availability badges: a HEAD request) and owns
  the cache folder the sidecar downloads into (`ROVES_CACHE_DIR` = this app's cache dir; the
  CLI keeps the zips in `shells/<version>/<asset>/`, the layout Packmaster's own downloader
  used, so the cache size/clear controls keep working). Since engine v0.9.0 the assets are the
  CEF players: `roves_shell_<platform>.zip`, `..._steam.zip`, and `..._mcp.zip` for every
  desktop platform (`shell::asset_name`; Steam + MCP is an error, the engine publishes no such
  package). The pin is v0.9.0, the first CEF player release (older tags are Servo shells with
  a different layout). That pinned tag is what a *real* Packmaster release always targets, for
  reproducibility — but a *test* build (`PACKMASTER_TEST_BUILD=1`, this project's own
  `test.yml`) targets whichever tag GitHub currently reports as the engine repo's own latest
  release instead (`resolve_shell_version()`, a live lookup against
  `api.github.com/repos/DRincs-Productions/roves/releases/latest`, falling back to
  `TARGET_SHELL_VERSION` if that lookup fails), and passes `--refresh` so the sidecar never
  reuses a cached download for it, since "latest" can change between runs.
- **`roves_cli.rs`** runs the sidecar: `roves bundle --json --progress json --platform <p>
  --content-dir --name <package name> --out-dir <work>/out --shell-version <v> [--steam
  --steam-app-id <id>] [--enable-mcp] [--icon-png <Release info icon>] [--no-extract]
  [--refresh]`, with no console window on Windows. Each `--progress json` stderr line becomes a
  `bundle-progress` event (`downloading` with its fraction, `assembling` 0.4, `extracting` ->
  `zipping` 0.6); other stderr lines are logs, and `roves: error: ...` is the error shown to
  the user. The JSON result gives the zip, the extracted folder (only requested for installers)
  and the player executable. Everything the CLI does — verification against
  `SHA256SUMS.txt`, zip-to-zip copy keeping modes/setuid/symlinks, the `game/` folder, the
  project icon (Release info icon, else the build's `icon.png`, else its
  `icon.ico`/`favicon.ico`, else the Roves icon; `play.exe` icon resource, macOS `.icns`),
  `steam_appid.txt`, the MCP folder rule, `PROVENANCE.txt` — is documented in roves-cli's
  README and the engine's `skills/core/SKILL.md`; it replaced Packmaster's former
  `cef_player.rs`/`icon.rs` (and their `image`/`icns`/`editpe`/`sha2`/`plist` crates).
  `real_sidecar` (ignored) is the end-to-end test of this path against a real sidecar and
  player zip.
- **`bundle.rs`** orchestrates per selected platform (and per format, for installers),
  emits `bundle-progress` events the frontend listens for, and moves the sidecar's zip to
  `release/<stem>_<platform>[_mcp].zip`: `<name>/...` on Windows/Linux, the renamed
  `<name>.app` directly at the archive root on macOS (Finder never shows what's inside it, so
  no extra wrapper folder). Installers use the sidecar's extracted folder on their own host
  (where modes and symlinks are restored).
- **`installer.rs`** wraps the extracted package into a real `.msi`/`.dmg`/`.deb`, porting
  `post_build_commands.py`'s `_wrap_windows_msi`/`_wrap_macos_dmg`/`_bundle_linux_deb` (and
  `support/windows/roves-bundle.wxs.mako` for the WiX source) to Rust; the `.deb` links
  `/usr/bin/<name>` to the player executable and installs `chrome-sandbox` as root 4755.
  Each format only works when Packmaster itself is running on its
  matching OS *and* that OS's native tool (WiX/`hdiutil`/`dpkg-deb`) is already installed —
  `check_installer_availability` is a real, live check for this (host OS + tool-on-PATH),
  not an assumption; `configure.tsx`'s installer cards disable themselves accordingly.

**iOS backend (`ios.rs`/`ios_signing.rs`):** mirrors the Android backend's shape exactly --
downloads the engine's own `support/ios/` WKWebView container + `resources/` branding assets
(published by `.github/workflows/ios.yml` to the same rolling "test" release `android.rs`
already targets), ports `support/ios/bundle.py`'s `stage()` to Rust, then runs XcodeGen +
`xcodebuild` (unsigned Simulator build by default, or a real signed `.ipa` export with
`ios_signing.rs`'s imported certificate/provisioning profile -- see that module's own doc
comment for why there's no `generate` option the way Android's `signing.rs` has one: an Apple
Distribution certificate must be countersigned by Apple itself, so only import exists here).
**Only available when Packmaster itself is running on macOS** (`check_ios_availability`) --
unlike Android's own history (see that module's doc comment on the NDK/Windows gap that later
closed), this is a permanent restriction with no workaround: Xcode/XcodeGen simply don't exist
elsewhere. `configure.tsx`'s iOS card and its own "Advanced settings" accordion item
(app name, bundle ID, signing) are gated on this real, live check, same pattern as Android's
own card and `check_installer_availability`. **Not verified end to end on a real macOS
machine** in the session that added this -- no Rust toolchain to `cargo check`/`cargo build`
against on this Windows machine at all (see this repo's own known toolchain gaps), let alone
Xcode; real verification is this repo's own CI (`.github/workflows/test.yml`, which builds
Packmaster's portable output on real runners, macOS included) plus, ultimately, someone
actually running a generated iOS build on a device/simulator.

**Steam plugin:** the engine publishes a Steam variant of every desktop player
(`roves_shell_<platform>_steam.zip`, with the Steamworks redistributable next to the
executable), so `configure.tsx`'s "Steam" panel is a real control: a `Switch`
(`settings.plugins.steam.enabled`) and an App ID `Input` (`settings.plugins.steam.appId`).
`bundle.rs` validates the App ID (non-empty, digits only) and passes `--steam --steam-app-id`;
roves-cli picks the `_steam` asset, checks the redistributable is there (and absent from a
standard asset) and writes the App ID into `steam_appid.txt` next to the executable (Valve's own convention for
testing outside the real Steam client; macOS: `Contents/MacOS/`).

**AI interaction (MCP):** `settings.plugins.mcp.enabled`, off by default, every desktop
platform: each desktop output uses `roves_shell_<platform>_mcp.zip` and is named
`<name>_<platform>_mcp.zip` (`mcp/` next to `play.exe`, in `Contents/Resources/` of the
`.app`, next to the Linux executable). The switch is disabled while Steam is on (enabling
Steam turns it off) or when no desktop platform is selected; the Rust side refuses Steam +
MCP too.

**Game icon:** a single source PNG (`settings.icon.path`), picked in the "Release info"
card via `@tauri-apps/plugin-dialog`'s `open()`. Desktop players follow roves-cli's project
icon rules (`--icon-png`; the same as the engine's and roves-action's):
Release info icon, else the build's `icon.png`, else the largest frame of its
`icon.ico`/`favicon.ico`; that PNG becomes the window `icon.png`, a generated `.ico` is
set as `play.exe`'s main icon group (roves-cli, on every host) (the old group,
which the engine CI set to the sample game's icon, is removed; version info and manifest
are kept), and a generated `.icns` (`icns` crate, never upscaled)
replaces the macOS bundle icon. With no project icon the Windows base's `roves.ico` is the
`play.exe` fallback and the macOS `.icns` stays the Roves one. Both containers come from
maintained encoders (`image`'s `IcoEncoder`, `icns`), nothing hand-written, so they work on
every host (all of this now runs inside the roves-cli sidecar). Android: the same
explicit/auto-detected `icon.png` is passed to `android.rs`'s
`AndroidBuildOptions.icon_png`, which replaces `res/mipmap/servo.*`.

**Testing without a local Tauri build:** `.github/workflows/test.yml` builds Packmaster
(portable output) on every push and publishes it to a rolling "test" GitHub Release,
mirroring the main engine repo's own `test.yml` — see this project's own README for
details. Prefer this over asking whoever's iterating on Packmaster to have a working
Rust/Tauri toolchain on hand.

**Cutting a real, versioned release:** `.github/workflows/release.yml` (triggered by pushing
a `v<major>.<minor>.<patch>` tag) builds and publishes a real Packmaster release — same
build steps as `test.yml`, but without `PACKMASTER_TEST_BUILD` set, so it targets `shell.rs`'s
pinned `TARGET_SHELL_VERSION` for a reproducible result, and publishes to a real tagged
GitHub Release instead of the rolling "test" one. Mirrors the engine repo's own
`release.yml`/its "Cutting a versioned release" `CLAUDE.md` section — same tag-push trigger,
same delete-and-republish retry procedure if a run fails. Bump `package.json`'s and
`src-tauri/{Cargo.toml,tauri.conf.json}`'s `version` fields to match before tagging — they
don't update themselves.
