//! The plugin as a native plugin file: `cargo x4` and `cargo +esp x4pro`
//! build it, `paperpaw-sign pack` makes the file (see `README.md`).

#![no_std]
#![no_main]

paperpaw_sdk::plugin!(my_plugin::MyPlugin);
