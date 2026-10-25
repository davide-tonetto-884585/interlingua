//! A PaperPaw plugin to start from, after the SDK's own sample
//! (`samples/hello-plugin` in PaperPaw's repository). Everything a plugin
//! usually needs, small:
//!
//! - an **action**, *My plugin*, listed in the tools menu with an icon;
//! - a **screen** it opens, usable by touch and by buttons alike;
//! - a **setting**, the name to greet, listed in the settings;
//! - an **event** it listens to: books opened, counted;
//! - **translations**, English and German.
//!
//! The logic (the count) is in [`logic`], without a screen or device in it,
//! so that `cargo test` can check it on a computer.

#![no_std]

extern crate alloc;

pub mod logic;

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::Cell;

use paperpaw_sdk::builtin::{actions, events, groups, menus};
use paperpaw_sdk::setting::{Setting, Text};
use paperpaw_sdk::ui::{self, UiStyle};
use paperpaw_sdk::{
    ActionSpec, Arg, Canvas, Command, Cx, Env, Gesture, GestureKind, Handled, Id, Manifest,
    MenuEntry, Params, Plugin, Rect, RegisterError, Registrar, Screen, ScreenSpec, SettingSpec,
    Version,
};

use crate::logic::Counter;

/// Ids start with the plugin's id: `my-plugin.…`. The id is the same in
/// `plugin/plugin.ini`, in the store's address and in the plugin's folder
/// on the reader.
pub const PLUGIN: Id = Id::new_static("my-plugin");
pub const OPEN: Id = Id::new_static("my-plugin.open");
pub const SCREEN: Id = Id::new_static("my-plugin.screen");
pub const NAME: Setting<Text> = Setting::new("my-plugin.name");

/// The plugin. State shared between its parts lives in `Rc`s it hands to
/// them when it registers.
#[derive(Debug, Default)]
pub struct MyPlugin;

/// How it works, by language (see `Manifest::how`). The store shows it on
/// the plugin's page; the same files go into the plugin's folder.
static HOW: [(&str, &str); 2] = [
    ("en", include_str!("../how/en.txt")),
    ("de", include_str!("../how/de.txt")),
];

impl Plugin for MyPlugin {
    fn manifest(&self) -> Manifest {
        // The version is the one in plugin/plugin.ini: keep them equal.
        Manifest::new(PLUGIN, "My plugin", Version::new(0, 1, 0))
            .description("A plugin to start from.")
            .how(&HOW)
            .uses(paperpaw_sdk::Uses::NOTHING)
    }

    fn register(&self, r: &mut Registrar) -> Result<(), RegisterError> {
        // Books opened since the start, counted by the event below and
        // shown by the screen.
        let opened = Rc::new(Cell::new(Counter::new()));

        // The action: what a menu entry, a button or a gesture runs.
        r.add_action(
            ActionSpec::new(OPEN, "my-plugin.title").handler(|_, _, out| {
                out.push(Command::OpenScreen(SCREEN, Params::new()));
            }),
        )?;
        r.add_menu_entry(MenuEntry::new(menus::TOOLS, OPEN).order(90))?;
        r.add_icon(OPEN, Cow::Borrowed("star"))?;

        // The screen, made each time it opens.
        let count = opened.clone();
        r.add_screen(ScreenSpec::new(
            SCREEN,
            "my-plugin.title",
            Box::new(move |env, _| Box::new(MyScreen::new(env, count.get().get()))),
        ))?;

        // The setting: shown in the settings, saved by the kernel.
        r.add_setting(
            SettingSpec::text(
                NAME.static_str(),
                "my-plugin.setting.name",
                "",
                groups::OTHER,
            )
            .description("my-plugin.setting.name.help"),
        )?;

        // The event: every reader emits it when a book opens.
        r.on(events::BOOK_OPENED, move |_, _reading, _| {
            let mut counter = opened.get();
            counter.increment();
            opened.set(counter);
        })?;

        r.add_translations(Cow::Borrowed(include_str!("../i18n/en.ini")))?;
        r.add_translations(Cow::Borrowed(include_str!("../i18n/de.ini")))?;
        Ok(())
    }
}

/// The greeting, under a header with a back arrow.
struct MyScreen {
    style: UiStyle,
    header: ui::Header,
    opened: u32,
}

impl MyScreen {
    fn new(env: &Env<'_>, opened: u32) -> Self {
        let style = UiStyle::new(env);
        Self {
            header: ui::Header::new(env, &style, true),
            style,
            opened,
        }
    }

    fn greeting(&self, env: &Env<'_>) -> alloc::string::String {
        let name = match env.settings().read(&NAME).trim() {
            "" => env.i18n().tr("my-plugin.someone"),
            name => name,
        };
        let count = i64::from(self.opened);
        env.i18n().plural(
            "my-plugin.greeting",
            count,
            &[("name", Arg::Str(name)), ("count", Arg::Int(count))],
        )
    }
}

impl Screen for MyScreen {
    fn draw(&mut self, canvas: &mut Canvas<'_>, env: &Env<'_>) {
        // What the screen draws with: canvas, environment and style.
        let mut ui = ui::Ui::new(canvas, env, &self.style);
        let title = env.i18n().tr("my-plugin.title");
        self.header.draw(&mut ui, title, None, None);
        let display = env.device().display;
        let top = self.header.area().height;
        // A header taller than a tiny screen leaves no room, not an
        // overflow.
        let area = Rect::new(0, top, display.width, display.height.saturating_sub(top));
        ui.message(area, &self.greeting(env));
    }

    /// Buttons: *confirm* or *back* close it; other actions (e.g. the
    /// menu button) go on to their global handlers.
    fn on_action(&mut self, action: &Id, cx: &mut Cx<'_, '_>) -> Handled {
        if *action == actions::CONFIRM || *action == actions::BACK {
            cx.command(Command::Back);
            return Handled::Yes;
        }
        Handled::No
    }

    /// Touch: the header's back arrow, or a tap anywhere else, closes it.
    fn on_gesture(&mut self, gesture: Gesture, cx: &mut Cx<'_, '_>) -> Handled {
        if gesture.kind != GestureKind::Tap {
            return Handled::No;
        }
        if let Some(action) = self.header.tap(cx.env(), &self.style, gesture.start, None) {
            cx.command(Command::run(action));
        } else {
            cx.command(Command::Back);
        }
        Handled::Yes
    }
}
