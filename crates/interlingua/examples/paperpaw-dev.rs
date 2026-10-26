//! paperpaw-dev for this plugin: PaperPaw's standard app with the plugin
//! added, run on emulated devices and checked.
//!
//! ```sh
//! cargo run -p interlingua --example paperpaw-dev -- check --plugin interlingua --quick
//! cargo run -p interlingua --example paperpaw-dev -- --help
//! ```
//!
//! The report is in `target/paperpaw-dev/report.html`.

fn main() -> std::process::ExitCode {
    paperpaw_dev::main(|app| app.plugin(interlingua::Interlingua))
}
