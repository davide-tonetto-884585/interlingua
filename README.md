# my-plugin

Punto di partenza per un plugin con codice per [PaperPaw](https://paperpaw.org)
(Xteink X4 e X4 Pro). Deriva da `samples/hello-native` e `samples/hello-plugin`
del repository di PaperPaw (Apache-2.0), con la stessa struttura di un
repository di plugin vero.

**Prima di tutto**, in tutto il progetto, sostituisci:

| Cosa | Dove |
|---|---|
| `my-plugin` (id) | `plugin/plugin.ini`, `plugin/store/store.ini`, `Cargo.toml` (due), `src/main.rs`, `crates/my-plugin/**`, `build.sh`, i due workflow in `.github/` |
| `my_plugin::MyPlugin` | `src/main.rs`, `crates/my-plugin/examples/`, `crates/my-plugin/tests/snapshots.rs` |
| `YOUR-NAME` | `plugin/store/store.ini` (due volte) |

L'id deve essere in minuscolo, con cifre e trattini, al massimo 40 caratteri, e deve
iniziare ogni id nel codice (`my-plugin.open`). La versione in `plugin/plugin.ini`
e nel `Manifest` deve essere la stessa.

## Struttura

| Percorso | Cos'è |
|---|---|
| `crates/my-plugin/src/lib.rs` | il plugin: azione, schermata, impostazione, evento |
| `crates/my-plugin/src/logic.rs` | logica pura, senza schermo: si testa con `cargo test` |
| `crates/my-plugin/i18n/`, `how/` | traduzioni e "come funziona" |
| `crates/my-plugin/tests/` | test: snapshot e flussi per l'emulatore |
| `src/main.rs` | il crate che produce il file nativo (`code.ppn`): una riga |
| `plugin/` | la cartella che installano i lettori; `plugin/store/` è solo per lo store |
| `build.sh`, `.cargo/`, `paperpaw-plugin.x`, `rust-toolchain.toml` | compilazione e firma, come nel sample |
| `.github/workflows/` | CI: `build.yml` (release firmata su tag `v*`), `paperpaw-dev.yml` (controlli) |

## Provarlo sul computer

Serve Rust (`rustup`), il resto lo scarica Cargo. La release di PaperPaw è
`0.5.0-beta.6`: se ne esce una nuova, cambia insieme SDK, `patch`,
`paperpaw-dev`, `rust-toolchain.toml` e `PAPERPAW` nel workflow.

```sh
cargo test -p my-plugin                       # logica + snapshot (`-p`: dalla radice un semplice `cargo test` guarderebbe solo il crate nativo)
PAPERPAW_BLESS=1 cargo test -p my-plugin --test snapshots   # la prima volta: crea le immagini; guardale prima del commit
cargo run -p my-plugin --example paperpaw-dev -- check --plugin my-plugin --quick
cargo run -p my-plugin --example paperpaw-dev -- flows --plugin my-plugin --flow crates/my-plugin/tests/flows
```

Il rapporto è in `target/paperpaw-dev/report.html`. Dall'albero di PaperPaw la
documentazione completa è `docs/plugins.md` (sezioni "Testing" e "Checking your plugin").

I percorsi dei comandi `paperpaw-dev` sono relativi alla cartella del crate, quindi
se un comando non trova `tests/snapshots` o `tests/flows`, lancialo da
`crates/my-plugin`.

## Compilare per il lettore

```sh
cargo x4                  # X4: riscv32imc-unknown-none-elf
cargo +esp x4pro          # X4 Pro: serve la toolchain `esp` (espup) e le librerie core della release
```

Per le istruzioni complete (librerie core dell'X4 Pro, firma, installazione via USB)
vedi `samples/hello-native/README.md` nel repository di PaperPaw.

## Pubblicare

1. Crea una chiave una volta: `paperpaw-sign key ~/.paperpaw/author.key`. Tienila fuori
   dal repository, con una copia sicura.
2. Metti il contenuto della chiave nel segreto `PAPERPAW_PLUGIN_KEY` del repository.
3. Alza `version` in `plugin/plugin.ini` (e nel `Manifest`), committa anche `Cargo.lock`,
   poi `git tag v0.1.0 && git push --tags`: la CI costruisce e firma i due file.
4. Carica su <https://paperpaw.org/submit> lo ZIP `*-upload.zip` e i due `.ppn` firmati.

La guida completa è <https://paperpaw.org/authors>.
