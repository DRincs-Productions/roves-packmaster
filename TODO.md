# TODO — cose da fare su Roves Packmaster (roves-ui)

Backlog di lavoro noto ma non ancora fatto sulla UI di Packmaster. Vedi anche il `CLAUDE.md`
di questo repo per le convenzioni (i18n obbligatoria su 9 lingue, `npm run check` prima di
chiudere una modifica) e il `TODO.md`/`CUSTOMIZATIONS.md` del motore `roves` per il lavoro
lato engine collegato (es. Android).

---

## 1. Un'unica icona selezionabile in "Informazioni sulla release", con auto-detect dal progetto

**Stato:** noto, non ancora iniziato — richiesto esplicitamente il 2026-09-01, da fare in un
giro successivo (non toccare ora).

Oggi la card "Icona del gioco" (`configure.tsx`, accordion `icon` nella sezione "Avanzate")
ha **due** picker separati — PNG (finestra/taskbar) e ICO (icona `.exe` di Windows) — dentro
`src/lib/settings.ts`'s `IconSettings`. Da fare:

- Ridurre a **una sola icona selezionabile** nella sezione "Informazioni sulla release" (in
  cima alla pagina, non più dentro l'accordion "Avanzate") — probabilmente un solo campo PNG,
  dato che l'ICO di Windows è un caso più di nicchia (vedi punto sotto sulle differenze).
- Se il progetto ha già un'icona rilevabile (stesso auto-detect già usato lato engine —
  `icon.png`/`favicon.ico` in `--content-dir`, vedi `CUSTOMIZATIONS.md` del motore, voce
  2026-08-27), va **mostrata automaticamente** appena si seleziona la cartella sorgente,
  senza che l'utente debba aprirla lui stesso da un file picker.
- Da decidere: se l'ICO Windows sparisce del tutto dalla UI (mach bundle lo supporta comunque
  via `--icon-ico`, resterebbe solo non esposto in Packmaster) o resta come opzione avanzata
  separata per chi lo vuole specificamente.

## Note

- **2026-09-01 — differenza tra le due icone attuali (chiarimento, non un problema):**
  - **Icona finestra/taskbar (PNG)** — l'icona vera e propria della finestra in esecuzione e
    della sua voce nella taskbar. Applicata **a runtime**: l'engine legge un file `icon.png`
    accanto al binario a ogni avvio (`headed_window.rs`'s `runtime_window_icon_bytes`), quindi
    funziona anche su uno shell prebuilt mai ricompilato per quel gioco specifico. Funziona su
    Windows/Linux.
  - **Icona `.exe` di Windows (ICO multi-dimensione)** — un'icona diversa e indipendente:
    quella che **Esplora file/Windows** mostra per il file `play.exe` stesso (nell'esplora
    risorse, prima ancora di avviarlo, nelle anteprime, se fissato alla barra delle
    applicazioni prima dell'esecuzione, ecc.) — non l'icona della finestra in esecuzione.
    Applicata **dopo il packaging**, patchando la risorsa icona incorporata nel file `.exe`
    stesso via `rcedit` (uno strumento esterno scaricato e cacheato). Solo Windows, perché è
    un concetto specifico del formato PE di Windows (i binari Linux/macOS non hanno risorse
    icona incorporate allo stesso modo).
- **2026-09-01 — perché macOS non supporta l'icona PNG (limite noto, non un bug):** su macOS
  l'icona Dock/app **non** è un'icona-finestra runtime come su Windows/Linux — viene letta dal
  bundle `.app` stesso, dal suo `Info.plist` (`CFBundleIconFile`) che punta a un file `.icns`
  incorporato nel bundle **al momento del packaging**, non modificabile a runtime con il
  meccanismo attuale. Il motore non ha mai avuto codice per generare/incorporare un `.icns`
  (verificato: nessun riferimento a `.icns`/`CFBundleIconFile` in tutto l'albero) — è stato
  lasciato fuori deliberatamente invece di tentare alla cieca una feature non verificabile su
  una macchina non-macOS, non una svista. `mach bundle --icon-png` su macOS stampa un warning
  e ignora l'input invece di fallire silenziosamente. Vedi `CUSTOMIZATIONS.md` del motore,
  voce "2026-08-26 — Runtime + post-build game icon", per il dettaglio completo.
