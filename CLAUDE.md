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

Some settings are only meaningful on a specific host OS (see `src/lib/platform.ts`'s own
comment — `.msi` needs Windows+WiX, `.dmg` needs macOS+`hdiutil`, `.deb` needs
Linux+`dpkg-deb`). Hide, don't just disable, options that can't actually work on the
current system — an unavailable option a user can still see and try to enable is worse
than one that isn't shown at all. (`configure.tsx`'s installers/plugins screens are
currently hidden entirely on this same principle — see "Real bundling backend" below for
why, not a host-OS distinction this time.)

## Real bundling backend

"Generate release" is real, not a mock — see `src-tauri/src/{shell,packer,bundle}.rs`:

- **`shell.rs`** downloads (and caches, per version+platform, under this app's cache dir)
  the engine's own prebuilt `roves_shell_<platform>.zip` release asset — never compiles
  one. `TARGET_SHELL_VERSION` here **must** stay in sync with `src/lib/shell-version.ts`'s
  constant of the same name — see the engine repo's own `CLAUDE.md`, "Cutting a versioned
  release" section, which is the authoritative place this sync obligation is documented.
- **`packer.rs`** places the user's content into the downloaded shell — either packed (by
  linking the engine repo's `roves-content-packer` crate directly as a Cargo library
  dependency, so packing happens in-process, no separate toolchain or sidecar binary
  needed) or plain-copied, matching `compression.enabled`. Mirrors `python/servo/
  post_build_commands.py`'s `_place_bundle_content`/`_write_launch_config`/
  `_resolve_window_title` in the engine repo — consult that file before changing this
  one, since the two must keep producing bundles the shipped engine binary can actually
  launch (see `ports/servoshell/desktop/bundle_launch.rs` for the runtime contract:
  `launch.json`'s schema, and where packed content must live relative to the binary).
- **`bundle.rs`** orchestrates both per selected platform, emits `bundle-progress` events
  the frontend listens for, and zips the result.

**Why portable only, and no Steam plugin, for now:** both need something this
download-a-prebuilt-shell approach doesn't have — native per-platform installer tooling
(WiX/`dpkg-deb`/`hdiutil`) for the former, a Steam-enabled prebuilt shell variant for the
latter (the engine's own `v0.1.0` release is a single, default-features build). Revisit
`configure.tsx`'s hidden installers/plugins sections once either becomes real, rather than
half-wiring them now.

**Testing without a local Tauri build:** `.github/workflows/test.yml` builds Packmaster
(portable output) on every push and publishes it to a rolling "test" GitHub Release,
mirroring the main engine repo's own `test.yml` — see this project's own README for
details. Prefer this over asking whoever's iterating on Packmaster to have a working
Rust/Tauri toolchain on hand.
