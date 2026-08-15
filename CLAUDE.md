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
than one that isn't shown at all.
