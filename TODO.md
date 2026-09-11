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

**Stato: fatto (2026-09-10), non verificato con un build reale.** Vedi `src-tauri/src/
signing.rs`'s own doc comment per il meccanismo completo, e il `TODO.md` del motore `roves`,
voce #5, per il lato engine/Gradle di questo stesso obiettivo (le 4 variabili d'ambiente
`APK_SIGNING_KEY_*` che questo modulo imposta prima di invocare `gradlew`).

Un'unica sezione "Firma della release" dentro la card Mobile (accordion "Impostazioni
avanzate"): genera un nuovo keystore (`keytool`, tramite lo stesso JRE portatile che Packmaster
già scarica per Gradle) oppure importane uno esistente (percorso via file picker + alias + le
due password). Le credenziali sono salvate in chiaro in un file JSON separato nella cartella
di configurazione dell'app (`android-signing.json`, **non** nel `settings.json` di progetto) —
una scelta deliberata e documentata, non un keychain OS reale (Credential Manager/Keychain/
Secret Service): stesso livello di fiducia di `~/.android/debug.keystore` o `~/.netrc`, non
cifrato a riposo ma mai trasmesso altrove. Un'integrazione keychain reale (es. crate `keyring`)
sarebbe un miglioramento, deliberatamente non tentato qui: porterebbe dipendenze native per
piattaforma non testabili in questa sessione (e su Linux dipende da un demone Secret Service
che potrebbe non essere in esecuzione).

**Non ancora fatto:** un vero build firmato non è mai stato eseguito ed è mai stato verificato
con `apksigner verify` o installando l'apk risultante su un dispositivo reale — solo il codice
Rust compila (`roves-packmaster` CI verde) e TypeScript/i18n sono validati.

## 3. Supporto Android su Windows

**Stato: sbloccato (2026-09-10), primo test reale su Windows il 2026-09-11 -- fallito, in
diagnosi.** Due gap indipendenti erano stati risolti il 2026-09-10: il fix lato motore
(`TODO.md` del motore, voce #4, `ndk-build.cmd`) e — scoperto solo dopo, leggendo `android.rs`
con più attenzione — il bootstrap JRE/SDK/NDK di **questo stesso file**, che semplicemente non
aveva mai avuto un ramo Windows (`adoptium_os_arch`/`sdk_os_tag`/`ndk_download_info`
restituivano tutti errore esplicito su Windows). Vedi il commento in testa a `android.rs` per
il dettaglio completo di entrambi.

**2026-09-11 — primo report reale:** un utente su una vera macchina Windows ha ottenuto
`sdkmanager exited with exit code: 1 while installing ["platform-tools",
"platforms;android-37", "build-tools;36.0.0"]` — nessun altro dettaglio, perché Packmaster è
un'app GUI senza console su ogni piattaforma: `run_sdkmanager`/`accept_sdk_licenses`/gradlew
usavano tutti `.status()` con stdio ereditata (che su Windows senza console non va da nessuna
parte visibile), quindi qualunque cosa `sdkmanager`/`gradlew` scrivessero sul VERO motivo del
fallimento (licenza rifiutata, problema di rete, versione Java incompatibile, ...) veniva
scartata in silenzio. **Non ancora diagnosticato il vero motivo** — prima cosa sistemata:
`run_capturing_output`/`format_process_failure` (nuovi helper in `android.rs`) catturano ora
stdout+stderr di ogni invocazione (`sdkmanager --licenses`, `sdkmanager <install>`, `gradlew`)
e li includono (troncati) nel messaggio d'errore mostrato in UI — così il prossimo tentativo
di questo stesso utente dovrebbe finalmente dire *perché* fallisce, invece del solo exit code.
Non ancora verificato: serve un secondo tentativo reale su quella stessa macchina Windows con
questa build per sapere la causa vera.

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
