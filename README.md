# Interlingua

Bilingual reading for [PaperPaw](https://paperpaw.org) (Xteink X4 and X4 Pro): a small
translation above each line of the book, or a translated paragraph after each paragraph.

It is based on PaperPaw's `samples/interlinear`, from release **0.5.0-beta.9** (plugin
interface 4), the first one with what it needs: room between lines asked for by a text
filter (`Block::leading_above`), word positions for layers (`PageView::words`), blocks
inserted by filters (`Block::insert_after`) and taps on layers (`PageLayerSpec::items`).

## How it works

You translate a book chapter by chapter ("Translate current chapter" in the reader menu),
with **Google** or **DeepL**, chosen in *Reading settings → Interlingua*. Each service needs
your own key (a secret setting, kept under "Passwords and keys"): no key is in the code.
Translations are kept on the card and the text filter reads them from there. When you turn
to a page that is not translated, the plugin asks how many pages to translate.

Google uses Chrome's unofficial endpoint (`translate-pa.googleapis.com`), which Google may
limit or shut down at any time; DeepL is the official API (free keys end in `:fx`). The text
of each paragraph is sent to the service you choose, and only when you ask for a translation.

## Layout

| Path | What it is |
|---|---|
| `crates/interlingua/src/lib.rs` | the plugin: settings, text filter, layer, taps |
| `crates/interlingua/src/text.rs` | sentences and words, pure logic with tests |
| `crates/interlingua/src/engine.rs` | picks the service and its key |
| `crates/interlingua/src/google.rs`, `deepl.rs` | request and answer of each service |
| `crates/interlingua/src/pairing.rs` | which translation goes with which sentence |
| `crates/interlingua/tests/` | the plugin on PaperPaw's reader, as on a device |
| `crates/interlingua/i18n/`, `how/` | interface translations (en, de, it) and "how it works" |
| `src/main.rs` | the crate that makes the file for readers (`code.ppn`): one line |
| `plugin/` | the folder readers install; `plugin/store/` is for the store only |
| `build.sh`, `.cargo/`, `paperpaw-plugin.x`, `rust-toolchain.toml` | building and signing, from beta.9 |
| `.github/workflows/paperpaw-dev.yml` | tests and checks on every push |
| `.github/workflows/paperpaw-release.yml` | on a `v*` tag: builds, signs, checks and releases |

## Getting started

You need Rust (`rustup`); Cargo downloads the rest, PaperPaw included.

```sh
cargo test -p interlingua          # logic + the plugin on PaperPaw's reader
cargo run -p interlingua --example paperpaw-dev -- check --plugin interlingua --quick
./emulator.sh                      # rebuilds and opens the desktop emulator
```

The checks' report is in `target/paperpaw-dev/report.html`.

## Building for the reader

```sh
cargo x4                  # X4: riscv32imc-unknown-none-elf
cargo +esp x4pro          # X4 Pro: `esp` toolchain (espup) and the release's core libraries
./build.sh --key ~/.paperpaw/author.key   # both, signed, in dist/ (Linux or macOS)
```

The reader needs PaperPaw **0.5.0-beta.9**: a plugin for interface 4 does not run on older
firmware. Full instructions: `samples/hello-native/README.md` in PaperPaw's repository.

## Licence

Apache-2.0 (`LICENSE`); where the parts come from is in `NOTICE`.
