# Diario di apprendimento

A fine sessione chiedi a Claude: "Riassumi i concetti Rust incontrati oggi con un esempio
dal mio codice" e incolla qui il risultato, con la data.

## Concetti da incontrare in questo progetto

Spunti di partenza, tutti già presenti nel codice. Segna quelli che ti sono chiari.

- [ ] `#![no_std]` e `extern crate alloc`: cosa cambia rispetto a `std`.
- [ ] `Rc<Cell<Counter>>`: perché condividere stato tra azione, schermata ed evento, e
      perché `Cell` e non `RefCell` (`logic.rs`, `lib.rs`).
- [ ] Closure che catturano (`move |env, _| ...`) e cosa significa `move` qui.
- [ ] Trait `Plugin` e `Screen`: cosa promette il compilatore a chi li implementa.
- [ ] `const fn` con `&mut self` e `saturating_add` (`logic.rs`).
- [ ] `#[derive(Copy, Clone)]` e quando un tipo può essere `Copy`.
- [ ] `include_str!` e dati incorporati nel binario.
- [ ] Lifetime anonimi in `Env<'_>`, `Canvas<'_>`, `Cx<'_, '_>`.
- [ ] Perché il crate nativo ha `#![no_main]` e una sola riga.

## Sessioni

<!-- Aggiungi qui sotto una sezione per ogni sessione. -->
