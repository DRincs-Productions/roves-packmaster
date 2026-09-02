# TODO — cose da fare su Roves Packmaster (roves-ui)

Backlog di lavoro noto ma non ancora fatto sulla UI di Packmaster. Vedi anche il `CLAUDE.md`
di questo repo per le convenzioni (i18n obbligatoria su 9 lingue, `npm run check` prima di
chiudere una modifica) e il `TODO.md`/`CUSTOMIZATIONS.md` del motore `roves` per il lavoro
lato engine collegato (es. Android).

---

## 1. Un'unica icona selezionabile in "Informazioni sulla release", con auto-detect dal progetto

**Stato: fatto (2026-09-02).** `configure.tsx` ora ha un solo campo icona (PNG) dentro
"Informazioni sulla release", con auto-detect di `icon.png` dalla cartella sorgente mostrato
automaticamente (non serve più aprire il file picker per vederlo). `settings.ts`/`settings.rs`
consolidati a `IconSettings { path }`. L'ICO di Windows non è sparito dalla UI: non serve più
fornirlo a parte, perché `src-tauri/src/icon.rs` lo **genera da solo** dalla stessa PNG
(`generate_ico`), insieme a un `.icns` per macOS (`generate_icns`) — vedi il `CLAUDE.md` di
questo repo, sezione "Game icon", per il dettaglio completo di dove viene applicata l'icona
piattaforma per piattaforma. macOS non è più un gap: prima non c'era alcun meccanismo di
icona per il Dock, ora c'è. Non verificato con una build reale (nessun ambiente per compilare
in questa sessione — vedi il commit per il disclaimer, stessa cautela già usata per il
backend Android).

## 2. Firma dell'APK Android generato da Packmaster

**Stato:** noto, non ancora iniziato — vedi il `TODO.md` del motore `roves`, voce #5, per il
lato engine/Gradle di questo stesso obiettivo.

Oggi `android.rs` produce solo un `.apk` di debug (firmato con il keystore di debug standard di
Gradle) — non distribuibile. Per un vero .apk di release, Packmaster dovrebbe offrire un modo
per l'utente di generare o importare un keystore reale (a differenza di `roves-action`, qui non
c'è un GitHub Secrets a cui appoggiarsi — l'utente non ha necessariamente `keytool`/Android
Studio installati, quindi probabilmente va generato con lo stesso Android SDK/JDK che Packmaster
già scarica da solo) e passarne le credenziali al task Gradle di release al posto di quello di
debug. Da decidere anche come/se persistere il keystore tra un utilizzo e l'altro senza
rischiare di esporne la chiave privata.

## 3. Supporto Android su Windows

**Stato:** bloccato lato engine — vedi il `TODO.md` del motore `roves`, voci #4 e #6.

`check_android_availability()` in `android.rs` disabilita esplicitamente l'opzione Android
quando Packmaster gira su Windows, perché il task Gradle che copia `libservoshell.so` invoca
`ndk-build` senza fallback `.cmd` (fallisce su Windows a prescindere da chi lo invoca). Nessuna
modifica possibile qui finché quel punto non è risolto lato motore (è un file Gradle
vendorizzato di Servo, non qualcosa che Packmaster possa aggirare da solo).

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
