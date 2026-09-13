# TODO — cose da fare su Roves Packmaster (roves-packmaster)

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

**Superato in gran parte (2026-09-13):** il motore è passato da Servo/JNI a WebView nativa
per Android (vedi `CUSTOMIZATIONS.md` del motore, voce "Mobile pivots from Servo to native
WebView") — `android.rs` non scarica/compila/strippa più alcuna libreria nativa, quindi non
c'è più alcun NDK da bootstrappare su nessuna piattaforma, Windows incluso. Tutto ciò che
riguarda NDK/`libservoshell.so`/`ndk-build.cmd`/`llvm-strip` qui sotto è storico: descrive un
problema che semplicemente non esiste più nella nuova architettura. Restano ancora validi:
la risoluzione dinamica del platform package (`resolve_platform_package`, il bug
`platforms;android-37` era reale e indipendente dall'NDK) e il fix JRE→JDK (Gradle continua a
servire un JDK vero). Non ancora verificato su Windows con la nuova architettura WebView-only
— dovrebbe essere più semplice da verificare ora (solo JDK+SDK+Gradle, niente NDK), ma nessun
ambiente Windows reale disponibile in questa sessione per confermarlo.

**Stato (storico, pre-pivot): sbloccato (2026-09-10), primo test reale su Windows il 2026-09-11 -- fallito, in
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
scartata in silenzio. Prima cosa sistemata: `run_capturing_output`/`format_process_failure`
(nuovi helper in `android.rs`) catturano ora stdout+stderr di ogni invocazione (`sdkmanager
--licenses`, `sdkmanager <install>`, `gradlew`) e li includono (troncati) nel messaggio
d'errore mostrato in UI.

**Causa reale trovata con il secondo tentativo (stesso giorno):** grazie all'output ora
visibile, l'errore vero era `Warning: Failed to find package 'platforms;android-37'`.
Primo fix (`--channel=3` su ogni chiamata a `sdkmanager`, mancante qui ma già presente
in `.github/workflows/android.yml` del motore) **necessario ma non sufficiente** — il terzo
tentativo ha dato lo stesso identico errore.

**Causa reale completa, trovata interrogando direttamente il repository Google (non
supposizione):** dal 2026-09 in poi Google versiona i platform level 37+ come
`platforms;android-37.0`/`.1`/`.2` (più prerelease `-betaN`) — **non esiste affatto** un
pacchetto letteralmente chiamato `platforms;android-37`. La stringa hardcoded
`format!("platforms;android-{ANDROID_PLATFORM}")` non ha mai potuto corrispondere a nulla,
indipendentemente dal canale. `android.yml` funziona perché non hardcoda mai il nome esatto:
risolve dinamicamente via `sdkmanager --list | grep -oE "platforms;android-37(\.[0-9]+)?
(-ext[0-9]+)?" | grep -v -- '-beta\|-rc' | sort -V | tail -1`.

**Fix:** nuova funzione `resolve_platform_package()` in `android.rs` che replica la stessa
logica (lista via `sdkmanager --channel=3 --list`, filtra prerelease, sceglie la versione
`.N`/`-extN` più alta) invece di costruire la stringa a mano.

**Quarto tentativo (stesso giorno) — un problema diverso, più avanti nel processo:**
la risoluzione SDK ha funzionato, ma `gradlew` è fallito con
`No Java compiler found, please ensure you are running Gradle with a JDK`. Causa:
`ensure_jre()` ha sempre scaricato letteralmente una **JRE** (Java Runtime Environment,
`.../jre/hotspot/...` sull'API di Adoptium) — una JRE non include `javac`, necessario a
Gradle 9.5.1 per il proprio codegen dei version catalog. `.github/workflows/android.yml`
del motore non ha mai avuto questo problema perché `actions/setup-java` installa di
default una vera JDK, non una JRE — una differenza facile da perdere portando lo stesso
bootstrap qui da zero. **Fix:** cambiato l'URL Adoptium da `/jre/` a `/jdk/`, e la cartella
di cache da `jre` a `jdk` (così chi ha già una cache vecchia, rotta, non continua a
riusarla all'infinito).

**Quinto tentativo (stesso giorno) — successo, ma con un problema serio scoperto dopo:**
la build Android è finalmente andata a buon fine, ma l'apk generato pesava **1,9 GB**
(`lib/arm64-v8a/libservoshell.so` da solo: 1,86 GB) contro un progetto sorgente ben più
piccolo — segnalato direttamente dall'utente. Causa: `roves_android_native_arm64.zip`,
scaricato da `download_native_library()`, è pubblicato da `.github/workflows/android.yml`
del motore, il cui stesso titolo dice "debug" — una build Rust `dev`-profile completamente
non spogliata (debug info DWARF piena), mai pensata per finire in un apk reale. Nessuna
ottimizzazione lato Gradle tocca questo file: AGP impacchetta byte per byte quello che
trova in `jniLibs`.

**Fix:** nuova `strip_native_library()` — usa l'`llvm-strip` già scaricato insieme
all'NDK (nessun tool aggiuntivo) con `--strip-all` sul file appena copiato in
`native_target_dir`, prima che Gradle lo veda. Verificato **con il file reale generato
dall'utente**, non solo a tavolino: 1,86 GB → 246 MB (con solo `--strip-debug` si scende
solo a 411 MB). Verificato anche che i simboli JNI (`Java_org_servo_servoview_JNIServo_*`,
risolti per nome a runtime, non via `RegisterNatives`) sopravvivono a `--strip-all` intatti
(`llvm-nm -D` sul file spogliato). 246 MB resta comunque grande per un `.so` mobile — è il
massimo ottenibile senza ricompilare in modalità release, cosa che Packmaster non fa mai di
proposito (nessun toolchain Rust richiesto). Il fix vero, più a monte, sarebbe far
pubblicare al motore una build native realmente in modalità release per l'embedding, non
riusare l'artefatto CI "debug" — non affrontato in questa sessione. Non ancora verificato
che l'apk stripped installi/funzioni davvero su un dispositivo reale, solo che si genera
alla dimensione attesa.

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
