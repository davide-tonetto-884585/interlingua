# Interlingua

Lettura bilingue per [PaperPaw](https://paperpaw.org) (Xteink X4 e X4 Pro): una piccola
traduzione sopra ogni riga del libro, oppure un paragrafo tradotto dopo ogni paragrafo.

Si basa su `samples/interlinear` di PaperPaw, dalla release **0.5.0-beta.9** (interfaccia
dei plugin 4), la prima con ciò che serve: spazio fra le righe chiesto da un filtro di testo
(`Block::leading_above`), posizione delle parole per i layer (`PageView::words`), blocchi
inseriti dai filtri (`Block::insert_after`) e tocchi sui layer (`PageLayerSpec::items`).

**Stato:** traduce capitolo per capitolo ("Traduci capitolo corrente") con **Google** o
**DeepL**, a scelta in *Impostazioni di lettura → Interlingua*. Ogni servizio vuole la
chiave dell'utente (impostazione segreta, salvata fra "Password e chiavi"): nel codice non
c'è nessuna chiave. Le traduzioni restano su scheda e il filtro le legge da lì.

Google usa l'endpoint non ufficiale di Chrome (`translate-pa.googleapis.com`), che Google può
limitare o chiudere; DeepL è l'API ufficiale (le chiavi gratuite finiscono con `:fx`).

## Struttura

| Percorso | Cos'è |
|---|---|
| `crates/interlingua/src/lib.rs` | il plugin: impostazioni, filtro di testo, layer, tocchi |
| `crates/interlingua/src/text.rs` | frasi e parole, logica pura con test |
| `crates/interlingua/src/engine.rs` | sceglie il servizio e la sua chiave |
| `crates/interlingua/src/google.rs`, `deepl.rs` | richiesta e risposta di ciascun servizio |
| `crates/interlingua/src/pairing.rs` | **esercizio**: quale traduzione va con quale frase |
| `crates/interlingua/tests/interlingua.rs` | il plugin sul reader di PaperPaw, come su un lettore |
| `crates/interlingua/i18n/`, `how/` | traduzioni dell'interfaccia (en, de, it) e "come funziona" |
| `src/main.rs` | il crate che produce il file per i lettori (`code.ppn`): una riga |
| `plugin/` | la cartella che installano i lettori; `plugin/store/` è solo per lo store |
| `build.sh`, `.cargo/`, `paperpaw-plugin.x`, `rust-toolchain.toml` | compilazione e firma, dalla beta.9 |
| `.github/workflows/paperpaw-dev.yml` | test e controlli a ogni push |
| `.github/workflows/paperpaw-release.yml` | su un tag `v*`: compila, firma, controlla e crea la release |

## Primo avvio

Serve Rust (`rustup`): il resto lo scarica Cargo, compreso PaperPaw dal suo repository.

```sh
cargo generate-lockfile          # la prima volta: poi committa Cargo.lock
cargo test -p interlingua         # logica + plugin sul reader di PaperPaw
cargo test -p interlingua -- --ignored   # i test dell'esercizio (falliscono finché non scrivi pair)
cargo run -p interlingua --example paperpaw-dev -- check --plugin interlingua --quick
```

Il rapporto dei controlli è in `target/paperpaw-dev/report.html`.

## Compilare per il lettore

```sh
cargo x4                  # X4: riscv32imc-unknown-none-elf
cargo +esp x4pro          # X4 Pro: toolchain `esp` (espup) e librerie core della release
./build.sh --key ~/.paperpaw/author.key   # entrambi, firmati, in dist/ (Linux o macOS)
```

Il lettore deve avere PaperPaw **0.5.0-beta.9**: un plugin per l'interfaccia 4 non gira su
firmware più vecchi. Istruzioni complete: `samples/hello-native/README.md` nel repository di
PaperPaw.

## Pubblicare da GitHub

1. Crea la chiave una volta: `paperpaw-sign key ~/.paperpaw/author.key`. Tienila fuori dal
   repository, con una copia sicura.
2. Mettine il contenuto nel segreto `PAPERPAW_PLUGIN_KEY` del repository.
3. Sostituisci `YOUR-NAME` in `plugin/store/store.ini` e aggiungi al repository il topic
   `paperpaw-plugin`.
4. Alza `version` in `plugin/plugin.ini` e nel `Manifest` (`lib.rs`), committa anche
   `Cargo.lock`, poi `git tag v0.1.0 && git push --tags`.

Il workflow controlla tutto con `paperpaw-sign check-publish` e pubblica la release;
paperpaw.org la elenca come "Community, unreviewed". In alternativa si carica a mano su
<https://paperpaw.org/submit>. Guida completa: <https://paperpaw.org/authors>.

## Prossimi passi

1. **Traduzione in background** di un capitolo (`paperpaw_sdk::work`).

## Licenza

Apache-2.0 (`LICENSE`); provenienza delle parti in `NOTICE`.
