# Roves Packmaster (roves-ui)

A Vite + React + Tauri desktop app that gives game developers a GUI for packaging their
built web content into Roves distributions (portable binaries, `.msi`/`.deb`/`.dmg`
installers) — without needing Rust or Python installed themselves. See `README.md` for
what it actually does today and what it doesn't yet.

Routing: [TanStack Router](https://tanstack.com/router), file-based (`src/routes/`).
Components: [shadcn](https://ui.shadcn.com/) (Base UI style, see `components.json`).
i18n: [i18next](https://www.i18next.com/)/`react-i18next` (`src/i18n/`).
Formatting/linting: [Biome](https://biomejs.dev/) — **not** Oxlint or ESLint.

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

"Generate release" is real, not a mock — see `src-tauri/src/{shell,packer,bundle}.rs`:

- **`shell.rs`** downloads (and caches, per version+platform, under this app's cache dir)
  the engine's own prebuilt `roves_shell_<platform>.zip` release asset — never compiles
  one. `TARGET_SHELL_VERSION` here **must** stay in sync with `src/lib/shell-version.ts`'s
  constant of the same name — see the engine repo's own `CLAUDE.md`, "Cutting a versioned
  release" section, which is the authoritative place this sync obligation is documented.
  That pinned tag is what a *real* Packmaster release always targets, for reproducibility —
  but a *test* build (`PACKMASTER_TEST_BUILD=1`, this project's own `test.yml`) targets
  whichever tag GitHub currently reports as the engine repo's own latest release instead
  (`resolve_shell_version()`, a live lookup against
  `api.github.com/repos/DRincs-Productions/roves/releases/latest`, falling back to
  `TARGET_SHELL_VERSION` if that lookup fails) — so testing Packmaster against a newly cut
  engine release doesn't need a `TARGET_SHELL_VERSION` bump here just to pick it up. (An
  earlier version of this pointed a test build at the engine's own rolling `test` tag
  instead, assuming it published a bare shell the way a real release does — it doesn't, so
  that found nothing published for any platform; "latest real release" is what actually
  exists to target.) The on-disk shell cache is also bypassed entirely for a test build
  (`ensure_shell`), since "latest" can change between runs.
- **`packer.rs`** places the user's content into the downloaded shell — either packed (by
  linking the engine repo's `roves-content-packer` crate directly as a Cargo library
  dependency, so packing happens in-process, no separate toolchain or sidecar binary
  needed) or plain-copied, matching `compression.enabled`. Mirrors `python/servo/
  post_build_commands.py`'s `_place_bundle_content`/`_write_launch_config`/
  `_resolve_window_title` in the engine repo — consult that file before changing this
  one, since the two must keep producing bundles the shipped engine binary can actually
  launch (see `ports/servoshell/desktop/bundle_launch.rs` for the runtime contract:
  `launch.json`'s schema, and where packed content must live relative to the binary).
  Content (the `.pack`/`manifest.json` archives, or the loose files when uncompressed)
  lands in a `content/` subfolder (`packer::CONTENT_SUBDIR`) rather than flat next to the
  binary — `bundle_launch.rs`'s `content_dir`/`url` fields are read as an arbitrary
  relative path already, so this needed no engine-side change, only pointing that path at
  a real subfolder instead of `""`. Keeps the bundle root down to just the engine's own
  files (play.exe/play, diagnose.bat/sh, launch.json, DLLs/dylibs) plus `steam_appid.txt`
  when Steam is enabled (has to stay flat — Valve's own convention for testing outside the
  real Steam client) — see `packer.rs`'s own tests for the exact file-layout contract this
  guards against regressing.
- **`bundle.rs`** orchestrates both per selected platform (and per format, for installers —
  see below), emits `bundle-progress` events the frontend listens for, and zips the
  portable output.
- **`installer.rs`** wraps the same staging dir `bundle.rs` already assembled into a real
  `.msi`/`.dmg`/`.deb`, porting `post_build_commands.py`'s `_wrap_windows_msi`/
  `_wrap_macos_dmg`/`_bundle_linux_deb` (and `support/windows/roves-bundle.wxs.mako` for
  the WiX source) to Rust. Each format only works when Packmaster itself is running on its
  matching OS *and* that OS's native tool (WiX/`hdiutil`/`dpkg-deb`) is already installed —
  `check_installer_availability` is a real, live check for this (host OS + tool-on-PATH),
  not an assumption; `configure.tsx`'s installer cards disable themselves accordingly.

**Steam plugin:** the engine now publishes a Steam-enabled shell variant alongside the plain
one (`roves_shell_<platform>_steam.zip` — see the engine repo's own `release.yml`/`CLAUDE.md`),
so `configure.tsx`'s "Steam" panel is a real control now, not informational-only: a `Switch`
(`settings.plugins.steam.enabled`) and an App ID `Input` (`settings.plugins.steam.appId`),
matching the Compression section's own toggle+fields pattern. `shell.rs`'s
`ensure_shell`/`is_shell_available` take a `steam: bool` and pick the matching asset (cached
separately per variant); `bundle.rs`'s `generate_release` validates the App ID (non-empty,
digits only) and writes it into a `steam_appid.txt` next to the packaged executable (Valve's
own convention for testing outside the real Steam client) — see `write_steam_appid`.

**Testing without a local Tauri build:** `.github/workflows/test.yml` builds Packmaster
(portable output) on every push and publishes it to a rolling "test" GitHub Release,
mirroring the main engine repo's own `test.yml` — see this project's own README for
details. Prefer this over asking whoever's iterating on Packmaster to have a working
Rust/Tauri toolchain on hand.
