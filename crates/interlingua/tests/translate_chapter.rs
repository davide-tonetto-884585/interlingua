//! "Translate current chapter" on PaperPaw's own reader: the action asks
//! for a paragraph at a time, each answer is kept on the card and the next
//! paragraph is asked for. The network is the test's: it takes the
//! requests the kernel hands to a shell and answers them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests panic on purpose when something is wrong"
)]

use interlingua::Interlingua;
use paperpaw_core::{Effects, HttpCall, HttpResponse, Io, Kernel};
use paperpaw_platform::{
    DeviceInfo, DisplayInfo, InputCaps, Instant, MemoryClass, PixelFormat, Rotation,
};
use paperpaw_sdk::builtin::events::{PAGE_LAYER_TAPPED, PageLayerTapped};
use paperpaw_sdk::ui::choice::Picked;
use paperpaw_sdk::builtin::{actions, params, slots};
use paperpaw_sdk::event::EventData;
use paperpaw_sdk::secrets::SecretChange;
use paperpaw_sdk::{Id, MemoryStorage, Params, read_file};

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

fn kernel(config: &str) -> Kernel {
    let storage = MemoryStorage::new()
        .with(BOOK, include_str!("books/alice-in-wonderland.txt"))
        .with(".paperpaw/plugins/setup/done", "");
    let builder = Kernel::builder(DEVICE)
        .storage(storage)
        .network(true)
        .user_config(format!("[help]\nshow = false\n{config}"));
    let mut kernel = paperpaw_app::install(builder, paperpaw_app::Fonts::Packs)
        .plugin(Interlingua)
        .build()
        .unwrap();
    if !config.contains("no-key") {
        kernel.set_secret(SecretChange::keep("interlingua.google-key", "Google", "TESTKEY"));
        kernel.set_secret(SecretChange::keep("interlingua.deepl-key", "DeepL", "DEEPKEY:fx"));
    }
    kernel
}

/// Runs what the kernel asked to be woken for, up to a second ahead.
fn settle(kernel: &mut Kernel, mut effects: Effects, now: &mut u64) {
    for _ in 0..10_000 {
        let Some(at) = effects.wake_at() else { return };
        if at.0 > *now + 1000 {
            return;
        }
        *now = (*now).max(at.0);
        effects = kernel.tick(Instant(*now));
    }
}

fn http_calls(kernel: &mut Kernel) -> Vec<HttpCall> {
    kernel
        .take_io()
        .into_iter()
        .filter_map(|io| match io {
            Io::Http(call) => Some(call),
            _ => None,
        })
        .collect()
}

#[test]
fn the_chapter_is_asked_for_a_paragraph_at_a_time_and_kept() {
    let mut kernel = kernel("");
    let mut now = 0;
    let effects = kernel.start(Instant(0));
    settle(&mut kernel, effects, &mut now);
    now += 100;
    let open = Params::new().with(params::PATH, BOOK);
    let effects = kernel.open_slot(Instant(now), slots::READER, open);
    settle(&mut kernel, effects, &mut now);
    kernel.take_io();

    translate_chapter(&mut kernel, &mut now);

    let mut asked = Vec::new();
    for n in 1..=3 {
        let calls = http_calls(&mut kernel);
        assert_eq!(calls.len(), 1, "request {n}: {calls:?}");
        let call = &calls[0];
        assert_eq!(call.method, "GET");
        assert!(
            call.url.starts_with("https://translate-pa.googleapis.com/v1/translate?"),
            "{}",
            call.url
        );
        assert!(call.url.contains("query.text="));
        assert!(call.url.contains("&key=TESTKEY&"), "{}", call.url);
        // The language chosen (Italian by default), not the reader's own.
        assert!(call.url.contains("query.target_language=it"), "{}", call.url);
        asked.push(call.url.clone());
        // The body goes to a file, as on a device and in the emulator,
        // where the reply cannot carry it to a plugin built apart.
        let file = call.save_to.clone().expect("the answer is saved to a file");
        assert_eq!(file, ".paperpaw/plugins/interlingua/answer.json");
        let body = format!(r#"{{"translation":"Prova {n}"}}"#).into_bytes();
        // The second answer as a shell gives it: written to the file, not
        // in the reply; the others in the reply.
        let saved = n == 2;
        if saved {
            kernel.storage().write(&file, &body).unwrap();
        }
        now += 100;
        let effects = kernel.http_response(
            Instant(now),
            HttpResponse {
                id: call.id,
                status: 200,
                body: if saved { Vec::new() } else { body },
                saved,
                ..HttpResponse::default()
            },
        );
        settle(&mut kernel, effects, &mut now);
    }
    asked.dedup();
    assert_eq!(asked.len(), 3, "each request asks for another paragraph");

    // What was answered is on the card, one line each.
    let dir = ".paperpaw/plugins/interlingua";
    let mut found = Vec::new();
    kernel
        .storage()
        .list(dir, &mut |entry| found.push(entry.name.to_string()))
        .unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("tr-") && found[0].ends_with(".0"), "{found:?}");
    let data = read_file(kernel.storage(), &format!("{dir}/{}", found[0]), 4096).unwrap();
    let data = String::from_utf8(data).unwrap();
    assert_eq!(data.lines().count(), 3, "{data}");
    for n in 1..=3 {
        assert!(data.contains(&format!("\tProva {n}\n")), "{data}");
    }
    // The file of the answer is gone once read.
    assert!(!kernel.storage().exists(&format!("{dir}/answer.json")));
}

/// The screen's gray values, a byte a pixel.
fn screen(kernel: &mut Kernel) -> Vec<u8> {
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

/// Two paragraphs translated, then an error and the panel closed: the
/// screens before and after, and the kernel.
fn two_kept_then_a_failure(config: &str) -> (Vec<u8>, Vec<u8>, Kernel, u64, String) {
    let mut kernel = kernel(config);
    let mut now = 0;
    let effects = kernel.start(Instant(0));
    settle(&mut kernel, effects, &mut now);
    now += 100;
    let open = Params::new().with(params::PATH, BOOK);
    let effects = kernel.open_slot(Instant(now), slots::READER, open);
    settle(&mut kernel, effects, &mut now);
    kernel.take_io();
    let before = screen(&mut kernel);

    translate_chapter(&mut kernel, &mut now);
    for n in 1..=2 {
        let call = http_calls(&mut kernel).remove(0);
        now += 100;
        let effects = kernel.http_response(
            Instant(now),
            HttpResponse {
                id: call.id,
                status: 200,
                body: format!(r#"{{"translation":"Prova {n}"}}"#).into_bytes(),
                ..HttpResponse::default()
            },
        );
        settle(&mut kernel, effects, &mut now);
    }
    // The third answer is an error: the job ends.
    let call = http_calls(&mut kernel).remove(0);
    now += 100;
    let effects = kernel.http_response(
        Instant(now),
        HttpResponse {
            id: call.id,
            status: 500,
            ..HttpResponse::default()
        },
    );
    settle(&mut kernel, effects, &mut now);

    assert!(http_calls(&mut kernel).is_empty(), "no request after an error");
    // The panel stays on the error until Back closes it; the page is
    // compared without it.
    now += 100;
    let effects = kernel.run_action(Instant(now), actions::BACK);
    settle(&mut kernel, effects, &mut now);
    assert!(!kernel.screens().any(|s| s.as_str() == "interlingua.progress"));
    let after = screen(&mut kernel);
    (before, after, kernel, now, call.url)
}

#[test]
fn a_failed_request_ends_the_job_and_the_page_shows_what_was_kept() {
    let (before, after, mut kernel, mut now, failed) =
        two_kept_then_a_failure("[interlingua]\nabove = off\nparallel = on\n");
    assert_ne!(before, after, "the page is laid out again with the kept translations");

    // Running it again goes on from the third paragraph, and asks for it.
    translate_chapter(&mut kernel, &mut now);
    let again = http_calls(&mut kernel);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].url, failed, "the paragraph that failed");
}

#[test]
fn the_kept_translations_are_drawn_above_the_lines() {
    let (before, after, ..) = two_kept_then_a_failure("[interlingua]\nabove = on\nparallel = off\n");
    assert_ne!(before, after, "the rooms above the lines show the translations");
}

#[test]
fn a_tap_above_a_line_opens_its_whole_translation() {
    let (_, _, mut kernel, mut now, _) =
        two_kept_then_a_failure("[interlingua]\nabove = on\nparallel = off\n");
    assert!(!kernel.screens().any(|s| s.as_str() == "interlingua.full"));

    // The first translated paragraph starts a line of the first page.
    let dir = ".paperpaw/plugins/interlingua";
    let mut files = Vec::new();
    kernel
        .storage()
        .list(dir, &mut |entry| files.push(entry.name.to_string()))
        .unwrap();
    let file = files.iter().find(|f| f.starts_with("tr-")).unwrap();
    let data = read_file(kernel.storage(), &format!("{dir}/{file}"), 4096).unwrap();
    let data = String::from_utf8(data).unwrap();
    let offset: u32 = data.split('\t').next().unwrap().parse().unwrap();

    // The reader says where the line starts, a few bytes past where its
    // block does (spaces, line breaks): tried from the block on.
    let event = Id::from(PAGE_LAYER_TAPPED);
    for at in offset..offset + 40 {
        let tapped = PageLayerTapped::new(Id::new_static("interlingua.layer"), BOOK.into(), 100, 60)
            .section(0)
            .offset(at)
            .item(0);
        now += 100;
        let effects = kernel.send_event(Instant(now), event.clone(), tapped.to_params());
        settle(&mut kernel, effects, &mut now);
        if kernel.screens().any(|s| s.as_str() == "interlingua.full") {
            break;
        }
    }
    assert!(
        kernel.screens().any(|s| s.as_str() == "interlingua.full"),
        "{:?}",
        kernel.screens().collect::<Vec<_>>()
    );
}

/// The book open, with the plugin on and its question asked for.
fn open_book(config: &str) -> (Kernel, u64) {
    let mut kernel = kernel(config);
    let mut now = 0;
    let effects = kernel.start(Instant(0));
    settle(&mut kernel, effects, &mut now);
    now += 100;
    let open = Params::new().with(params::PATH, BOOK);
    let effects = kernel.open_slot(Instant(now), slots::READER, open);
    settle(&mut kernel, effects, &mut now);
    kernel.take_io();
    (kernel, now)
}

fn on_top(kernel: &Kernel, id: &str) -> bool {
    kernel.screens().last().is_some_and(|s| s.as_str() == id)
}

/// "Translate current chapter" from the menu, answered with the whole
/// chapter.
fn translate_chapter(kernel: &mut Kernel, now: &mut u64) {
    *now += 100;
    let effects = kernel.run_action(Instant(*now), Id::new_static("interlingua.translate"));
    settle(kernel, effects, now);
    assert!(on_top(kernel, "core.choice-dialog"), "{:?}", kernel.screens().collect::<Vec<_>>());
    pick(kernel, now, "chapter");
}

/// Answers the question with `option`.
fn pick(kernel: &mut Kernel, now: &mut u64, option: &str) {
    *now += 100;
    let picked = Picked::new(BOOK.into(), option.into());
    let event = Id::new_static("interlingua.picked");
    let effects = kernel.send_event(Instant(*now), event, picked.to_params());
    settle(kernel, effects, now);
    if on_top(kernel, "core.choice-dialog") {
        *now += 100;
        let effects = kernel.run_action(Instant(*now), actions::BACK);
        settle(kernel, effects, now);
    }
}

/// Turns a page and lets the kernel settle.
fn next_page(kernel: &mut Kernel, now: &mut u64) {
    *now += 100;
    let effects = kernel.run_action(Instant(*now), actions::NEXT_PAGE);
    settle(kernel, effects, now);
}

#[test]
fn a_page_without_translations_asks_what_to_translate_and_no_is_remembered() {
    let (mut kernel, mut now) = open_book("[interlingua]\nabove = off\nparallel = on\n");
    next_page(&mut kernel, &mut now);
    assert!(on_top(&kernel, "core.choice-dialog"), "{:?}", kernel.screens().collect::<Vec<_>>());

    // No: back to the book, and not asked again on the next pages.
    pick(&mut kernel, &mut now, "no");
    next_page(&mut kernel, &mut now);
    next_page(&mut kernel, &mut now);
    assert!(!on_top(&kernel, "core.choice-dialog"), "asked again after a no");

    // Asked for by hand, it asks again (and then the pages after).
    now += 100;
    let effects = kernel.run_action(Instant(now), Id::new_static("interlingua.translate"));
    settle(&mut kernel, effects, &mut now);
    assert!(on_top(&kernel, "core.choice-dialog"));
}

#[test]
fn some_pages_are_translated_from_the_page_shown_and_stop_at_the_limit() {
    let (mut kernel, mut now) = open_book("[interlingua]\nabove = off\nparallel = on\n");
    next_page(&mut kernel, &mut now);
    next_page(&mut kernel, &mut now);
    assert!(on_top(&kernel, "core.choice-dialog"));
    pick(&mut kernel, &mut now, "5");
    assert!(on_top(&kernel, "interlingua.progress"));

    // The first paragraph asked for is on the page shown, not the start of
    // the book.
    let calls = http_calls(&mut kernel);
    assert_eq!(calls.len(), 1);
    assert!(!calls[0].url.contains("query.text=%5BIllustration%5D"), "{}", calls[0].url);

    // Answer until the limit: about five pages of paragraphs, not the
    // chapter's hundred.
    let mut asked = 0;
    let mut call = calls.into_iter().next();
    while let Some(c) = call.take() {
        asked += 1;
        let file = c.save_to.clone().unwrap();
        kernel.storage().write(&file, br#"{"translation":"Prova"}"#).unwrap();
        now += 100;
        let effects = kernel.http_response(
            Instant(now),
            HttpResponse { id: c.id, status: 200, saved: true, ..HttpResponse::default() },
        );
        settle(&mut kernel, effects, &mut now);
        call = http_calls(&mut kernel).into_iter().next();
        assert!(asked < 100, "it did not stop");
    }
    assert!((3..60).contains(&asked), "{asked} paragraphs for five pages");
    assert!(!on_top(&kernel, "interlingua.progress"), "the panel closes when it is done");
}

#[test]
fn deepl_is_asked_when_chosen_with_its_own_key() {
    let (mut kernel, mut now) = open_book("[interlingua]\nabove = off\nparallel = on\nengine = interlingua.engine.deepl\n");
    translate_chapter(&mut kernel, &mut now);
    let calls = http_calls(&mut kernel);
    assert_eq!(calls.len(), 1, "{calls:?}");
    let call = &calls[0];
    assert_eq!(call.method, "POST");
    assert_eq!(call.url, "https://api-free.deepl.com/v2/translate");
    assert!(call.headers.iter().any(|(n, v)| n == "Authorization" && v == "DeepL-Auth-Key DEEPKEY:fx"));
    let body = call.body.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    assert!(body.contains(r#""target_lang":"IT""#), "{body}");

    // Its answer is kept as the paragraph's translation.
    let file = call.save_to.clone().unwrap();
    kernel
        .storage()
        .write(&file, br#"{"translations":[{"detected_source_language":"EN","text":"Prova DeepL"}]}"#)
        .unwrap();
    now += 100;
    let effects = kernel.http_response(
        Instant(now),
        HttpResponse { id: call.id, status: 200, saved: true, ..HttpResponse::default() },
    );
    settle(&mut kernel, effects, &mut now);
    let dir = ".paperpaw/plugins/interlingua";
    let mut files = Vec::new();
    kernel.storage().list(dir, &mut |entry| files.push(entry.name.to_string())).unwrap();
    let file = files.iter().find(|f| f.starts_with("tr-")).unwrap();
    let kept = read_file(kernel.storage(), &format!("{dir}/{file}"), 4096).unwrap();
    let kept = String::from_utf8(kept).unwrap();
    assert!(kept.contains("Prova DeepL"), "{kept:?}");
}

#[test]
fn without_a_key_nothing_is_asked_or_sent() {
    let (mut kernel, mut now) = open_book("# no-key\n[interlingua]\nabove = off\nparallel = on\n");
    // By the menu: told to enter the key, no question.
    now += 100;
    let effects = kernel.run_action(Instant(now), Id::new_static("interlingua.translate"));
    settle(&mut kernel, effects, &mut now);
    assert!(!on_top(&kernel, "core.choice-dialog"));
    assert!(http_calls(&mut kernel).is_empty());
    // By a page turned to: not asked either.
    next_page(&mut kernel, &mut now);
    assert!(!on_top(&kernel, "core.choice-dialog"));
}
