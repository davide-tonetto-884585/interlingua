//! The plugin on PaperPaw's own reader, as on a device: a book opened on
//! its fifth page, drawn with each way switched on, and compared. After
//! the test of PaperPaw's `samples/interlinear`.
//!
//! The book is Project Gutenberg's *Alice's Adventures in Wonderland*
//! (public domain in the USA; its licence is at the end of the file), as
//! PaperPaw's `samples/library` has it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests panic on purpose when something is wrong"
)]

use interlingua::Interlingua;
use paperpaw_core::{Effects, Kernel};
use paperpaw_platform::{
    DeviceInfo, DisplayInfo, InputCaps, Instant, MemoryClass, PixelFormat, Rotation,
};
use paperpaw_sdk::builtin::{params, slots};
use paperpaw_sdk::{MemoryStorage, Params};

/// A small reader with buttons and touch, of the X4's memory class.
const DEVICE: DeviceInfo = DeviceInfo {
    id: "test-both",
    name: "Test both",
    display: DisplayInfo {
        width: 480,
        height: 800,
        dpi: 220,
        format: PixelFormat::Gray4,
        rotation: Rotation::Deg0,
        color: None,
    },
    input: InputCaps {
        buttons: true,
        touch: true,
        pen: false,
        keys: &["back", "confirm", "left", "right", "up", "down", "power"],
    },
    memory: MemoryClass::Small,
};

const BOOK: &str = "books/alice-in-wonderland.txt";

/// The book open on its fifth page with `config`, drawn: the screen's
/// gray values, a byte a pixel.
fn page(config: &str) -> Vec<u8> {
    let storage = MemoryStorage::new()
        .with(BOOK, include_str!("books/alice-in-wonderland.txt"))
        .with(".paperpaw/plugins/setup/done", "");
    let builder = Kernel::builder(DEVICE)
        .storage(storage)
        .user_config(format!("[help]\nshow = false\n{config}"));
    let mut kernel = paperpaw_app::install(builder, paperpaw_app::Fonts::Packs)
        .plugin(Interlingua)
        .build()
        .unwrap();
    let mut now = 0;
    // Runs what the kernel asked to be woken for, up to a second ahead.
    let settle = |kernel: &mut Kernel, effects: Effects, now: &mut u64| {
        let mut effects = effects;
        for _ in 0..10_000 {
            let Some(at) = effects.wake_at() else { return };
            if at.0 > *now + 1000 {
                return;
            }
            *now = (*now).max(at.0);
            effects = kernel.tick(Instant(*now));
        }
    };
    let effects = kernel.start(Instant(0));
    settle(&mut kernel, effects, &mut now);
    now += 100;
    let open = Params::new().with(params::PATH, BOOK);
    let effects = kernel.open_slot(Instant(now), slots::READER, open);
    settle(&mut kernel, effects, &mut now);
    for _ in 0..4 {
        now += 100;
        let effects = kernel.run_action(Instant(now), paperpaw_sdk::builtin::actions::NEXT_PAGE);
        settle(&mut kernel, effects, &mut now);
    }
    let mut screen = vec![255u8; 480 * 800];
    kernel.render(&mut |y, view| {
        for row in 0..view.height {
            for x in 0..view.width {
                screen[usize::from(y + row) * 480 + usize::from(x)] = view.pixel(x, row).luma();
            }
        }
    });
    screen
}

#[test]
fn both_ways_draw_and_change_the_page() {
    let plain = page("[interlingua]\nabove = off\nparallel = off\nask = off\n");
    let above = page("[interlingua]\nabove = on\nparallel = off\nask = off\n");
    let parallel = page("[interlingua]\nabove = off\nparallel = on\nask = off\n");
    assert_ne!(plain, above);
    // Nothing was translated and kept yet (see tests/translate_chapter.rs
    // for that), so the paragraphs after paragraphs have nothing to show.
    assert_eq!(plain, parallel);
}
