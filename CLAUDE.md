# Istruzioni per Claude in questo progetto

Questo è un plugin per il firmware PaperPaw (Xteink X4 e X4 Pro), scritto in Rust.

## Chi sono e cosa voglio

Sono un professionista che conosce sintassi e basi di Rust e vuole impararlo a fondo
scrivendo plugin reali. Il mio obiettivo è **capire**, non solo ottenere codice che funziona.
Rispondi in italiano; identificatori, commenti nel codice e messaggi dell'interfaccia
restano in inglese, come nel resto del progetto.

## Come lavorare con me

- **Prima di ogni modifica** spiega cosa cambi e perché, in poche righe. Poi mostra la
  modifica, piccola e leggibile.
- Se esistono più soluzioni idiomatiche (ownership, lifetime, `Rc`/`Cell`, trait, gestione
  degli errori, `no_std` e allocazioni), elencale con i compromessi e motiva la scelta.
- Segnala ogni concetto Rust non banale che incontriamo, con un esempio dal mio codice.
- Se ti chiedo di provare io, **non anticipare la soluzione**: dai un suggerimento e spiegami
  l'errore del compilatore invece di correggerlo.
- A richiesta, fammi un esercizio mirato sul concetto appena visto, o 3 domande per
  verificare se ho capito.
- Un passo alla volta: niente riscritture di interi file se basta una modifica.

## Regole tecniche del progetto

- La libreria del plugin è `#![no_std]` con `alloc`. Su X4 l'heap totale è di circa 262 KB:
  evita allocazioni inutili e dimmi quando una scelta ha un costo di memoria.
- La logica va in moduli puri (come `logic.rs`) testabili con `cargo test`; il codice che
  disegna o tocca il dispositivo resta sottile.
- Gli id iniziano con l'id del plugin (`my-plugin.…`). La versione in `plugin/plugin.ini`
  e nel `Manifest` deve essere la stessa.
- Ogni testo mostrato all'utente passa dalle traduzioni (`i18n/en.ini` e `de.ini`, stesse
  chiavi in entrambi): niente stringhe scritte nel codice.
- Non toccare `paperpaw-plugin.x`, `.cargo/config.toml` e `rust-toolchain.toml` senza
  dirmelo: fanno parte dell'interfaccia dei plugin della release.
- Non leggere, stampare o committare la chiave di firma (`*.key`). Non inventare
  fingerprint, chiavi o SHA-256: si leggono con `paperpaw-sign plugin <file>` e `sha256sum`.

## Prima di dichiarare finito un lavoro

`cargo clippy -p my-plugin --all-targets`, `cargo test -p my-plugin` e `cargo run -p my-plugin --example paperpaw-dev
-- check --plugin my-plugin --quick`: riporta l'esito, anche se ci sono avvisi.

## Riferimenti

- Guida per gli autori: <https://paperpaw.org/authors>
- Repository di PaperPaw: `docs/plugins.md`, `samples/hello-native`, `samples/hello-plugin`.
