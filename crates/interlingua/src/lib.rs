//! Interlingua: bilingual reading in PaperPaw, a small translation above
//! each line of a book, or a translated paragraph after each one.
//!
//! Built on PaperPaw's `samples/interlinear` (Apache-2.0), from release
//! 0.5.0-beta.9, the first with what an interlinear reading needs:
//!
//! - a **text filter** asks for room above every line of a paragraph
//!   (`Block::leading_above`, hundredths of an em), which the layout adds
//!   to each line's height, so pages break with it; and it may insert a
//!   translated paragraph after each one (`Block::insert_after`), which
//!   shows but takes no place in the book (bookmarks and highlights stay
//!   the book's);
//! - a **page layer** asks for the page's words (`.words()`), draws above
//!   each line the translation of each sentence part on it, starting
//!   where the part's first word starts, and says where its items are
//!   (`.items()`), so a tap (or Select, chosen with the buttons as links
//!   are) opens a line's whole translation and closes it again.
//!
//! The parts without a screen live in modules of their own, tested with
//! `cargo test`: [`text`] (sentences), [`google`] and [`deepl`] (the
//! requests and answers of the two services, chosen in [`engine`]) and
//! [`pairing`] (which translation goes with which sentence: an exercise).

#![no_std]

extern crate alloc;

pub mod cache;
pub mod chapter;
pub mod deepl;
pub mod engine;
pub mod full;
pub mod google;
pub mod look;
pub mod pairing;
pub mod paragraphs;
pub mod progress;
pub mod text;

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use paperpaw_sdk::builtin::fonts as builtin_fonts;
use paperpaw_sdk::builtin::{events, groups, menus, params};
use paperpaw_sdk::document::{Block, BlockKind, Locator, Run as TextRun, TextStyle};
use paperpaw_sdk::event::Event;
use paperpaw_sdk::filters::{FilterContext, TextFilterSpec, Verdict};
use paperpaw_sdk::ui::choice::Picked;
use paperpaw_sdk::library;
use paperpaw_sdk::layers::{LayerItem, PageLayerSpec, PageView};
use paperpaw_sdk::setting::{Setting, Text};
use paperpaw_sdk::ui::{self, UiStyle};
use paperpaw_sdk::{
    ActionSpec, Canvas, Color, Command, Env, Id, Manifest, MenuEntry, Params, Plugin, Rect,
    RegisterError, Registrar, ScreenSpec, SettingExtra, SettingGroupSpec, SettingSpec, Version,
};

use crate::progress::Progress;
use crate::paragraphs::{Missing, Paragraph, Sentence, Shown};

/// Ids start with the plugin's id: `interlingua.…`. The same id is in
/// `plugin/plugin.ini`, in the store's address and in the plugin's folder
/// on the reader.
pub const PLUGIN: Id = Id::new_static("interlingua");
/// A translation above each line.
pub const ABOVE: Setting<bool> = Setting::new("interlingua.above");
/// A translated paragraph after each paragraph.
pub const PARALLEL: Setting<bool> = Setting::new("interlingua.parallel");
/// The service that translates: Google or DeepL, each with the reader's
/// own key (secret settings, kept with the passwords and keys).
pub const ENGINE: Setting<Text> = Setting::new("interlingua.engine");
const ENGINES: [&str; 2] = [engine::GOOGLE, engine::DEEPL];
static GOOGLE_ONLY: SettingExtra = SettingExtra::new()
    .secret()
    .shown_if(Id::new_static("interlingua.engine"), engine::GOOGLE);
static DEEPL_ONLY: SettingExtra = SettingExtra::new()
    .secret()
    .shown_if(Id::new_static("interlingua.engine"), engine::DEEPL);

/// The language the book is translated into: one of [`TARGETS`].
pub const TO: Setting<Text> = Setting::new("interlingua.to");
const LAYER: Id = Id::new_static("interlingua.layer");
const FILTER: Id = Id::new_static("interlingua.filter");

/// Room above each line, in hundredths of an em (PaperPaw takes at most
/// four ems): the setting's percent of the text's size.
fn leading(env: &Env<'_>) -> u16 {
    number(env, &ROOM, 75)
}

/// The languages to translate into: the setting's values, each the
/// translation key of the language's name; Google's code follows
/// `interlingua.to.`.
const TARGETS: [&str; 12] = [
    "interlingua.to.de",
    "interlingua.to.en",
    "interlingua.to.es",
    "interlingua.to.fr",
    "interlingua.to.it",
    "interlingua.to.ja",
    "interlingua.to.nl",
    "interlingua.to.pl",
    "interlingua.to.pt",
    "interlingua.to.ru",
    "interlingua.to.tr",
    "interlingua.to.zh-CN",
];
const DEFAULT_TARGET: &str = "interlingua.to.it";

/// Ask what to translate when a page without translations is turned to.
pub const ASK: Setting<bool> = Setting::new("interlingua.ask");

/// The plugin's page in the reading settings.
const GROUP: Id = Id::new_static("interlingua.group");
static SETTINGS_GROUP: SettingGroupSpec =
    SettingGroupSpec::new(GROUP, "interlingua.title").within(groups::READING);

/// The translation's size above the lines, in percent of the room.
pub const SIZE: Setting<Text> = Setting::new("interlingua.size");
const SIZES: [&str; 7] = [
    "interlingua.size.30",
    "interlingua.size.40",
    "interlingua.size.50",
    "interlingua.size.60",
    "interlingua.size.70",
    "interlingua.size.80",
    "interlingua.size.90",
];
/// The room above each line, in percent of the text's size.
pub const ROOM: Setting<Text> = Setting::new("interlingua.room");
const ROOMS: [&str; 5] = [
    "interlingua.room.60",
    "interlingua.room.75",
    "interlingua.room.100",
    "interlingua.room.125",
    "interlingua.room.150",
];
/// The font of the translation above the lines: PaperPaw's or the book's.
pub const FONT: Setting<Text> = Setting::new("interlingua.font");
const FONTS: [&str; 4] = [
    "interlingua.font.ui",
    "interlingua.font.book",
    "interlingua.font.italic",
    "interlingua.font.bold",
];
/// How dark the translation is: the gray's value (0 black).
pub const SHADE: Setting<Text> = Setting::new("interlingua.shade");
const SHADES: [&str; 8] = [
    "interlingua.shade.0",
    "interlingua.shade.80",
    "interlingua.shade.130",
    "interlingua.shade.blue",
    "interlingua.shade.red",
    "interlingua.shade.green",
    "interlingua.shade.brown",
    "interlingua.shade.purple",
];
/// Whether the icon after a translation is drawn (and takes taps).
pub const ICON: Setting<bool> = Setting::new("interlingua.icon");
/// The style of the paragraph translated after a paragraph.
pub const PSTYLE: Setting<Text> = Setting::new("interlingua.pstyle");
const PSTYLES: [&str; 3] = [
    "interlingua.pstyle.italic",
    "interlingua.pstyle.regular",
    "interlingua.pstyle.bold",
];
/// What a closed line shows: all its translation, or its first word.
pub const CLOSED: Setting<Text> = Setting::new("interlingua.closed");
const CLOSED_WAYS: [&str; 2] = ["interlingua.closed.full", "interlingua.closed.first"];

static SETTINGS: [SettingSpec; 15] = [
    SettingSpec::switch(ABOVE.static_str(), "interlingua.above", true, GROUP)
        .description("interlingua.above.help"),
    SettingSpec::switch(PARALLEL.static_str(), "interlingua.parallel", false, GROUP)
        .description("interlingua.parallel.help"),
    SettingSpec::switch(ASK.static_str(), "interlingua.ask", true, GROUP)
        .description("interlingua.ask.help"),
    SettingSpec::choice(ENGINE.static_str(), "interlingua.engine", &ENGINES, engine::GOOGLE, GROUP)
        .description("interlingua.engine.help"),
    SettingSpec::text(engine::GOOGLE_KEY, "interlingua.google-key", "", GROUP)
        .with_extra(&GOOGLE_ONLY)
        .description("interlingua.google-key.help"),
    SettingSpec::text(engine::DEEPL_KEY, "interlingua.deepl-key", "", GROUP)
        .with_extra(&DEEPL_ONLY)
        .description("interlingua.deepl-key.help"),
    SettingSpec::choice(TO.static_str(), "interlingua.to", &TARGETS, DEFAULT_TARGET, GROUP)
        .description("interlingua.to.help"),
    SettingSpec::choice(SIZE.static_str(), "interlingua.size", &SIZES, "interlingua.size.70", GROUP)
        .description("interlingua.size.help"),
    SettingSpec::choice(ROOM.static_str(), "interlingua.room", &ROOMS, "interlingua.room.75", GROUP)
        .description("interlingua.room.help"),
    SettingSpec::choice(FONT.static_str(), "interlingua.font", &FONTS, "interlingua.font.ui", GROUP)
        .description("interlingua.font.help"),
    SettingSpec::choice(
        SHADE.static_str(),
        "interlingua.shade",
        &SHADES,
        "interlingua.shade.80",
        GROUP,
    )
    .description("interlingua.shade.help"),
    SettingSpec::choice(
        CLOSED.static_str(),
        "interlingua.closed",
        &CLOSED_WAYS,
        "interlingua.closed.full",
        GROUP,
    )
    .description("interlingua.closed.help"),
    SettingSpec::switch(ICON.static_str(), "interlingua.icon", true, GROUP)
        .description("interlingua.icon.help"),
    SettingSpec::choice(
        PSTYLE.static_str(),
        "interlingua.pstyle",
        &PSTYLES,
        "interlingua.pstyle.italic",
        GROUP,
    )
    .description("interlingua.pstyle.help"),
    SettingSpec::number(STAMP.static_str(), "interlingua.stamp", 0, MOST_STAMP, 0, GROUP)
        .with_extra(&NEVER_SHOWN)
        .description("interlingua.stamp.help"),
];

/// Changed each time a chapter's translations were kept: part of the text
/// filter's signature. A setting, so that changing it tells the reader
/// (`SETTING_CHANGED`), which lays the page out again with them; never
/// shown.
pub const STAMP: Setting<i64> = Setting::new("interlingua.stamp");
/// The stamp goes round below this, within the signature's bits.
pub const MOST_STAMP: i64 = 1 << 30;
/// Shown only while "Translation above lines" is `never`: never.
static NEVER_SHOWN: SettingExtra =
    SettingExtra::new().shown_if(Id::new_static("interlingua.above"), "never");

/// The question of what to translate: some pages from the page shown, or
/// the whole chapter; and, when it comes up by itself, not to now.
fn ask_how_much(env: &Env<'_>, path: &str, by_itself: bool) -> Command {
    let title = match by_itself {
        true => env.i18n().tr("interlingua.ask.page"),
        false => env.i18n().tr("interlingua.ask.menu"),
    };
    let mut options: Vec<&str> = PAGE_OPTIONS.to_vec();
    options.push(CHAPTER);
    if by_itself {
        options.push(NONE);
    }
    paperpaw_sdk::ask::choice(title, &PICKED)
        .about(path)
        .options("interlingua.option.", &options)
        .command()
}

/// The number at the end of a choice's value (`interlingua.size.70` is 70),
/// or `default`.
fn number(env: &Env<'_>, setting: &Setting<Text>, default: u16) -> u16 {
    env.settings()
        .read(setting)
        .rsplit('.')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(default)
}

/// Google's code of the language the book is translated into (`it`).
fn target(env: &Env<'_>) -> String {
    let to = env.settings().read(&TO);
    String::from(to.strip_prefix("interlingua.to.").unwrap_or("it"))
}

/// The file with the kept translation of the paragraph at `offset` of
/// `section` of the book at `path` (and of those near it), in the
/// language chosen.
fn cache_file(env: &Env<'_>, path: &str, section: u32, offset: u32) -> String {
    cache::path(
        &Env::plugin_dir("interlingua"),
        &library::key(path),
        &target(env),
        section,
        offset,
    )
}

/// How it works, by language (see `Manifest::how`). The store shows it on
/// the plugin's page; the same files are in `plugin/how/`.
static HOW: [(&str, &str); 3] = [
    ("en", include_str!("../how/en.txt")),
    ("de", include_str!("../how/de.txt")),
    ("it", include_str!("../how/it.txt")),
];

/// The menu entry (an action) that translates the chapter being read.
const TRANSLATE: Id = Id::new_static("interlingua.translate");

/// Where the reader is: shared by the handlers of the reader's events and
/// of the action that translates the chapter.
#[derive(Debug, Default)]
struct Run {
    /// The book, the section and where in it the page shown last starts.
    here: Option<(String, u32, u32)>,
    /// The reader said no (or went back) to the question asked when a page
    /// without translations was turned to: not asked again until a
    /// translation is asked for by hand.
    declined: bool,
    /// How many bytes of the book a page holds, as the last page drawn
    /// had: what "some pages" is in the book's own offsets.
    page_span: u32,
}

impl Run {
    /// A page's size in the book's offsets: the last page drawn's, else a
    /// guess.
    fn span(&self) -> u32 {
        match self.page_span {
            0 => DEFAULT_PAGE_SPAN,
            span => span,
        }
    }
}

/// The answer to what to translate.
const PICKED: Event<Picked> = Event::new("interlingua.picked");

/// Pages offered to translate, and the answer for a whole chapter and for
/// none (the options of the question: `interlingua.option.` and these).
const PAGE_OPTIONS: [&str; 3] = ["5", "10", "20"];
const CHAPTER: &str = "chapter";
const NONE: &str = "no";

/// A page's size in the book's offsets until one is drawn.
const DEFAULT_PAGE_SPAN: u32 = 1500;

type Shared = Rc<RefCell<Run>>;


/// The sentences drawn above each line, by where the line starts (and
/// where each sentence starts): what a tap on a line opens. The layer is
/// also drawn for pages not shown (the next one, made ready; a preview),
/// so the lines of the last few pages stay, [`MOST_LINES`] at most.
/// Filled by the layer as it draws, read by the handler of its taps
/// (`Rc`: two owners; `RefCell`: changed through a shared reference,
/// checked while it runs).
type Drawn = Rc<RefCell<Vec<(Locator, Vec<u32>)>>>;

/// Lines remembered for taps: a few pages' worth.
const MOST_LINES: usize = 64;

/// The plugin.
#[derive(Debug, Default)]
pub struct Interlingua;

impl Plugin for Interlingua {
    fn manifest(&self) -> Manifest {
        // The version is the one in plugin/plugin.ini: keep them equal.
        Manifest::new(PLUGIN, "Interlingua", Version::new(0, 1, 0))
            .description("A translation above each line, or after each paragraph.")
            .how(&HOW)
            .uses(paperpaw_sdk::Uses::NETWORK)
    }

    fn register(&self, r: &mut Registrar) -> Result<(), RegisterError> {
        r.add_setting_group(&SETTINGS_GROUP)?;
        r.add_settings(&SETTINGS)?;
        // The translations kept on the card go with their book (moved,
        // removed and cleaned up by PaperPaw).
        r.add_book_cache(Env::plugin_dir("interlingua"), cache::PREFIX)?;
        let loaded = RefCell::new(Loaded::default());
        // The translated paragraphs the filter met, for the layer.
        let kept = Rc::new(RefCell::new(Shown::default()));
        let filled = kept.clone();
        // The paragraphs seen without a translation, for the question.
        let lost = Rc::new(RefCell::new(Missing::default()));
        let noted = lost.clone();
        let run = Shared::default();
        r.add_text_filter(TextFilterSpec::block(
            FILTER,
            "interlingua.title",
            Box::new(move |env, at, block| {
                filter(env, at, block, &loaded, &filled, &noted);
                Verdict::Keep
            }),
            // Changes whenever what the filter does changes: pages are
            // laid out anew (the reader keys its pages with it).
            Box::new(move |env: &Env<'_>| {
                let s = env.settings();
                u64::from(s.read(&ABOVE))
                    | (u64::from(s.read(&PARALLEL)) << 1)
                    | ((s.read(&STAMP) as u64) << 2)
                    | ((TARGETS.iter().position(|t| *t == s.read(&TO)).unwrap_or(0) as u64) << 40)
                    | (u64::from(leading(env)) << 48)
                    | ((PSTYLES.iter().position(|t| *t == s.read(&PSTYLE)).unwrap_or(0) as u64) << 56)
            }),
        ))?;
        r.add_action(ActionSpec::new(TRANSLATE, "interlingua.translate").handler({
            let run = run.clone();
            move |env, _, out| {
                // By hand: asked how much, and the question when a page
                // is turned to may be asked again.
                let mut run = run.borrow_mut();
                let Some((path, ..)) = run.here.clone() else {
                    out.push(Command::Notify(env.i18n().tr("interlingua.no-book").into()));
                    return;
                };
                if !engine::ready(env) {
                    out.push(Command::Notify(env.i18n().tr("interlingua.no-key").into()));
                    return;
                }
                run.declined = false;
                out.push(ask_how_much(env, &path, false));
            }
        }))?;
        // The answer: the chapter, or some pages from the page shown, on a
        // panel of its own.
        r.on(PICKED, {
            let run = run.clone();
            move |_, picked, out| {
                let run = run.borrow();
                let Some((path, section, page)) = run.here.clone() else {
                    return;
                };
                let (from, until) = match picked.option.as_str() {
                    CHAPTER => (0, u32::MAX),
                    NONE => return,
                    pages => {
                        let pages: u32 = pages.parse().unwrap_or(5);
                        let span = run.span();
                        (page, page.saturating_add(pages.saturating_mul(span)))
                    }
                };
                out.push(Command::OpenScreen(
                    progress::SCREEN,
                    Params::new()
                        .with(params::PATH, path)
                        .with(params::SECTION, section.to_string())
                        .with(params::OFFSET, from.to_string())
                        .with(progress::UNTIL, until.to_string()),
                ));
            }
        })?;
        r.add_screen(ScreenSpec::new(
            progress::SCREEN,
            "interlingua.progress.title",
            Box::new(|env, params| Box::new(Progress::new(env, params))),
        ))?;
        r.add_menu_entry(MenuEntry::new(menus::READER_MORE, TRANSLATE).order(45))?;
        // Where the reader is: the chapter to translate is the one of the
        // page shown.
        for (event, turned) in [(events::BOOK_OPENED, false), (events::PAGE_TURNED, true)] {
            let (run, kept, lost) = (run.clone(), kept.clone(), lost.clone());
            r.on(event, move |env, reading, out| {
                let mut run = run.borrow_mut();
                run.here = Some((reading.path.clone(), reading.section, reading.offset));
                // The paragraphs near the page shown stay for the layer.
                kept.borrow_mut().near(reading.section, reading.offset);
                // A page turned to with paragraphs not translated: asked
                // what to do, once, until a translation is asked for.
                let s = env.settings();
                let wanted = s.read(&ASK)
                    && (s.read(&ABOVE) || s.read(&PARALLEL))
                    && engine::ready(env);
                let to = reading.offset.saturating_add(run.span());
                let page = (reading.section, reading.offset, to);
                let missing = lost.borrow().between(&reading.path, page.0, page.1, page.2);
                if wanted && missing && !run.declined && turned {
                    run.declined = true;
                    out.push(ask_how_much(env, &reading.path, true));
                }
            })?;
        }
        let gone = run.clone();
        r.on(events::BOOK_CLOSED, move |_, _, _| {
            gone.borrow_mut().here = None;
        })?;
        let drawn = Drawn::default();
        let filling = drawn.clone();
        let tapped_kept = kept.clone();
        let layer_run = run.clone();
        let item_kept = kept.clone();
        r.add_page_layer(
            PageLayerSpec::new(LAYER, "interlingua.title", move |canvas, env, page| {
                // The page drawn is the one shown: the paragraphs near it
                // are those to keep.
                kept.borrow_mut().near(page.start.section, page.start.offset);
                if page.end.section == page.start.section && page.end.offset > page.start.offset {
                    layer_run.borrow_mut().page_span = page.end.offset - page.start.offset;
                }
                draw(canvas, env, page, &kept.borrow(), &filling);
            })
            .switch(&ABOVE)
            .words()
            .items(move |env, page| items(env, page, &item_kept.borrow())),
        )?;
        // A tap on the room above a line (or Select on it) shows the
        // whole translation of the line's sentences.
        r.on(events::PAGE_LAYER_TAPPED, move |_, tapped, out| {
            if tapped.layer != LAYER || tapped.item.is_none() {
                return;
            }
            let Some(line) = tapped.line() else {
                return;
            };
            let drawn = drawn.borrow();
            let Some((_, starts)) = drawn.iter().find(|(start, _)| *start == line) else {
                return;
            };
            let kept = tapped_kept.borrow();
            let parts: Vec<&str> = starts
                .iter()
                .filter_map(|&at| {
                    let paragraph = kept.at(&tapped.path, line.section, at)?;
                    Some(paragraph.sentence_at(at)?.translation.as_str())
                })
                .filter(|t| !t.is_empty())
                .collect();
            if !parts.is_empty() {
                let text = parts.join(" ");
                let params = Params::new().with(params::TEXT, text);
                out.push(Command::OpenScreen(full::SCREEN, params));
            }
        })?;
        r.add_screen(ScreenSpec::new(
            full::SCREEN,
            "interlingua.full.title",
            Box::new(|env, params| Box::new(full::Full::new(env, params))),
        ))?;
        r.add_translations(Cow::Borrowed(include_str!("../i18n/en.ini")))?;
        r.add_translations(Cow::Borrowed(include_str!("../i18n/de.ini")))?;
        r.add_translations(Cow::Borrowed(include_str!("../i18n/it.ini")))?;
        Ok(())
    }
}

/// The cache file the filter read last, kept in memory: a section's
/// translations (at most [`cache::MAX_BYTES`]) instead of a read from the
/// card for every paragraph. `size` is the file's size when it was read:
/// when it changes, something was appended and the file is read again.
#[derive(Debug, Default)]
struct Loaded {
    path: String,
    size: u64,
    data: String,
}

/// The kept translation of the paragraph at `at`, if there is one.
fn cached(env: &Env<'_>, at: &FilterContext<'_>, loaded: &RefCell<Loaded>) -> Option<String> {
    if at.path.is_empty() {
        return None;
    }
    let storage = env.storage();
    let file = cache_file(env, at.path, at.at.section, at.at.offset);
    let size = storage.size(&file).ok()?;
    let mut loaded = loaded.borrow_mut();
    if loaded.path != file || loaded.size != size {
        let data = cache::load(storage, &file)?;
        *loaded = Loaded { path: file, size, data };
    }
    cache::lookup(&loaded.data, at.at.offset)
}

/// Room above each paragraph's lines, and its kept translation: split by
/// sentence for the layer above the lines (`shown`), and as a paragraph
/// after it.
fn filter(
    env: &Env<'_>,
    at: &FilterContext<'_>,
    block: &mut Block,
    loaded: &RefCell<Loaded>,
    shown: &RefCell<Shown>,
    lost: &RefCell<Missing>,
) {
    if block.kind != BlockKind::Paragraph || block.text.is_empty() {
        return;
    }
    let s = env.settings();
    let (above, parallel) = (s.read(&ABOVE), s.read(&PARALLEL));
    if above {
        block.leading_above = Some(leading(env));
    }
    // Only a translation that was kept is shown: a paragraph not yet
    // translated stays as it is, until "Translate current chapter".
    if !above && !parallel {
        return;
    }
    let Some(text) = cached(env, at, loaded) else {
        lost.borrow_mut().note(at.path, at.at.section, block.start, block.end);
        return;
    };
    lost.borrow_mut().done(at.path, at.at.section, block.start);
    if above {
        // Paired here, while the page is laid out, not while it is drawn.
        let paragraph = Paragraph::new(&block.text, &text, block.start, block.end, |i| {
            block.source_offset(i)
        });
        shown.borrow_mut().keep(at.path, at.at.section, paragraph);
    }
    if parallel {
        let len = text.len() as u32;
        let runs = match s.read(&PSTYLE) {
            "interlingua.pstyle.regular" => Vec::new(),
            "interlingua.pstyle.bold" => alloc::vec![TextRun { start: 0, end: len, style: TextStyle::BOLD }],
            _ => alloc::vec![TextRun { start: 0, end: len, style: TextStyle::ITALIC }],
        };
        // Past its bounds (at most four blocks, 4 KiB of text) it is left
        // out: a very long paragraph shows without its translation.
        let _ = block.insert_after(Block::new(BlockKind::Paragraph, 0, len, text).runs(runs));
    }
}

/// What is drawn above the lines: each sentence's translation flowed over
/// the rooms of its lines.
struct Placed {
    /// The line's index in the page.
    line: usize,
    x: i32,
    size: f32,
    /// How the text is inked.
    shade: Color,
    /// The text as drawn, cut to its room.
    text: String,
    /// Where the sentence starts in the book.
    sentence: u32,
    /// The icon after the last piece of a sentence's translation, the only
    /// part that takes a tap: where it starts and how wide it is (0 for the
    /// pieces before the last, which have none).
    icon_x: i32,
    icon_width: f32,
}

/// The icon's size, as a multiple of the translation's.
const ICON_SCALE: f32 = 1.5;

/// The icons tried for what opens a translation: the first the panel
/// can draw.
const ICONS: [&str; 4] = ["language", "message-dots", "info-circle", "dots"];

/// The icon after a translation, if one can be drawn.
fn tap_icon(env: &Env<'_>) -> Option<&'static str> {
    ICONS.into_iter().find(|name| ui::has_icon(env, name))
}

/// The part of a translation that takes a tap: the icon at `x`, `icon`
/// pixels wide, with a margin, but never narrower than a finger's tip;
/// as high as the room above the line.
fn tap_area(x: i32, icon: f32, top: u16, height: u16) -> Rect {
    let width = (icon + 8.0).max(MOST_NARROW) as u16;
    Rect::new((x - 4).max(0) as u16, top, width, height)
}

/// The smallest a translation is drawn, in pixels: below it a font pack
/// has no drawing.
const MOST_SMALL: f32 = 6.0;

/// The narrowest a tap's area is, in pixels.
const MOST_NARROW: f32 = 32.0;

/// The font of the translation above the lines.
fn font_of<'a>(env: &Env<'a>) -> Option<paperpaw_sdk::FontHandle<'a>> {
    // A reading role by its binding: the user's choice or the family's
    // face, which is what the book is set in.
    let role = match env.settings().read(&FONT) {
        "interlingua.font.book" => Some(builtin_fonts::READING),
        "interlingua.font.italic" => Some(builtin_fonts::READING_ITALIC),
        "interlingua.font.bold" => Some(builtin_fonts::READING_BOLD),
        _ => None,
    };
    role.and_then(|role| env.fonts().role(role.as_str()))
        .or_else(|| UiStyle::new(env).font(env))
}

/// Everything drawn above the lines of `page`: for each line, the part of
/// each sentence's translation that goes over it. Both the drawing and
/// the places that take taps come from it.
fn place(env: &Env<'_>, page: &PageView<'_>, shown: &Shown) -> Vec<Placed> {
    let Some(font) = font_of(env) else {
        return Vec::new();
    };
    let size_percent = f32::from(number(env, &SIZE, 70)) / 100.0;
    let shade = look::tone(env.settings().read(&SHADE));
    let first_word_only = env.settings().read(&CLOSED) == "interlingua.closed.first";
    let icon = tap_icon(env).filter(|_| env.settings().read(&ICON));

    // Every room above a line, in reading order: from where a sentence's
    // part on the line starts to where the next one's does.
    let mut rooms: Vec<Room<'_>> = Vec::new();
    for (index, line) in page.lines.iter().enumerate().filter(|(_, l)| l.leading > 0) {
        let first = rooms.len();
        let right = line.area.right() as i32;
        let words = page
            .words
            .iter()
            .filter(|w| w.start >= line.start && w.start < line.end);
        for word in words {
            let at = word.start;
            let Some(sentence) = shown
                .at(page.path, at.section, at.offset)
                .and_then(|p| p.sentence_at(at.offset))
            else {
                continue;
            };
            let on_line = &mut rooms[first..];
            if on_line
                .last()
                .is_some_and(|r| core::ptr::eq(r.sentence, sentence))
            {
                continue;
            }
            let x = i32::from(word.area.x);
            if let Some(before) = on_line.last_mut() {
                before.right = x;
            }
            rooms.push(Room {
                line: index,
                x,
                right,
                sentence,
                from: at.offset,
            });
        }
    }
    // Each sentence's translation flows over its rooms on the page.
    let mut placed = Vec::new();
    let mut start = 0;
    while start < rooms.len() {
        let sentence = rooms[start].sentence;
        let end = start
            + rooms[start..]
                .iter()
                .take_while(|r| core::ptr::eq(r.sentence, sentence))
                .count();
        let group = &rooms[start..end];
        start = end;
        let size = (f32::from(page.lines[group[0].line].leading) * size_percent).max(MOST_SMALL);
        let shade = look::readable(shade, size);
        // The icon takes room at the end of each piece.
        let icon_width = icon.map_or(0.0, |name| ui::icon_width(env, name, size * ICON_SCALE));
        let gap = if icon.is_some() { icon_width + 4.0 } else { 0.0 };
        let widths: Vec<f32> = group
            .iter()
            .map(|r| (r.right - r.x - 4) as f32 - gap)
            .map(|w| w.max(0.0))
            .collect();
        let text = sentence.part(group[0].from, sentence.end);
        // Each room's share of the text: as far as the sentence's part on
        // its line reaches, in the same proportion.
        let span = u64::from(sentence.end.saturating_sub(group[0].from)).max(1);
        let ends: Vec<usize> = group
            .iter()
            .enumerate()
            .map(|(n, _)| {
                let to = group.get(n + 1).map_or(sentence.end, |r| r.from);
                let done = u64::from(to.saturating_sub(group[0].from)).min(span);
                (done * text.len() as u64 / span) as usize
            })
            .collect();
        let pieces = paragraphs::flow(text, &widths, &ends, |t| font.measure(size, t));
        // One icon for the sentence's translation, after its last piece:
        // where it ends, or where it is cut for want of room.
        let last = pieces.iter().rposition(|p| !p.is_empty());
        for (n, ((room, piece), width)) in group.iter().zip(pieces).zip(widths).enumerate() {
            if piece.is_empty() {
                continue;
            }
            let first = piece.split_once(' ').map(|(first, _)| first);
            // Only the first word, with the icon (or "…" where there is
            // none) after it, when that is set; else all the piece.
            let shown_text = match (first_word_only, first, icon) {
                (true, Some(first), Some(_)) => String::from(first),
                (true, Some(first), None) => format!("{first} …"),
                _ => String::from(piece),
            };
            let text = ui::fit(&font, size, &shown_text, width);
            let text_width = font.measure(size, &text);
            placed.push(Placed {
                line: room.line,
                x: room.x,
                size,
                shade,
                sentence: sentence.start,
                icon_x: room.x + text_width as i32 + 4,
                icon_width: if icon.is_some() && last == Some(n) { icon_width } else { 0.0 },
                text,
            });
        }
    }
    placed
}

/// Above each line, the translation of each sentence part on it from
/// where its first word starts, each sentence's translation flowing over
/// the rooms of its lines (or only the first word and "…", as set). What
/// it draws above each line goes into `drawn` for a tap.
///
/// It reads nothing from storage: a layer draws from memory alone, on
/// some readers once for every band of the page.
fn draw(
    canvas: &mut Canvas<'_>,
    env: &Env<'_>,
    page: &PageView<'_>,
    shown: &Shown,
    drawn: &RefCell<Vec<(Locator, Vec<u32>)>>,
) {
    let Some(font) = font_of(env) else {
        return;
    };
    let icon = tap_icon(env);
    let mut drawn = drawn.borrow_mut();
    for p in place(env, page, shown) {
        let line = &page.lines[p.line];
        let baseline = i32::from(line.area.y) - i32::from(line.leading) / 4;
        font.draw(canvas, p.size, p.x as f32, baseline, &p.text, p.shade);
        if let Some(name) = icon.filter(|_| p.icon_width > 0.0) {
            // What a tap on this icon opens: this sentence.
            remember(&mut drawn, line.start, p.sentence);
            let top = line.area.y.saturating_sub(line.leading);
            let room = Rect::new(0, top, line.area.right() as u16, line.leading);
            ui::draw_icon(canvas, env, name, p.size * ICON_SCALE, p.icon_x as f32, room, p.shade);
        }
    }
}

/// What takes a tap: only the icon after each translation above a line,
/// not the words or the rest of the room (so that the page can be turned
/// and the menu opened with taps on the text).
fn items(env: &Env<'_>, page: &PageView<'_>, shown: &Shown) -> Vec<LayerItem> {
    let label = env.i18n().tr("interlingua.item");
    place(env, page, shown)
        .iter()
        .enumerate()
        .filter(|(_, p)| p.icon_width > 0.0)
        .map(|(i, p)| {
            let line = &page.lines[p.line];
            let top = line.area.y.saturating_sub(line.leading);
            let area = tap_area(p.icon_x, p.icon_width, top, line.leading);
            LayerItem::new(i as u32, area).label(label)
        })
        .collect()
}

/// Adds the sentence starting at `sentence` to those drawn above the line
/// starting at `line`, once; the oldest line goes past [`MOST_LINES`].
fn remember(drawn: &mut Vec<(Locator, Vec<u32>)>, line: Locator, sentence: u32) {
    match drawn.iter_mut().find(|(start, _)| *start == line) {
        Some((_, starts)) if starts.contains(&sentence) => {}
        Some((_, starts)) => starts.push(sentence),
        None => {
            if drawn.len() >= MOST_LINES {
                drawn.remove(0);
            }
            drawn.push((line, alloc::vec![sentence]));
        }
    }
}

/// The room above a line for a sentence's part on it.
struct Room<'s> {
    /// The line's index in the page.
    line: usize,
    /// Where it starts and ends on the screen.
    x: i32,
    right: i32,
    sentence: &'s Sentence,
    /// Where the sentence's part on the line starts in the book.
    from: u32,
}

#[cfg(test)]
mod tap_tests {
    use super::*;

    #[test]
    fn a_tap_takes_the_icon_not_the_words_or_the_room() {
        // An icon 24 pixels wide after a translation 300 pixels wide.
        let area = tap_area(240, 24.0, 100, 18);
        assert_eq!((area.x, area.y, area.height), (236, 100, 18));
        assert_eq!(area.width, 32);
        assert!(area.x > 40 + 150, "it does not reach back over the words");
    }

    #[test]
    fn a_small_icon_still_takes_a_finger() {
        assert_eq!(tap_area(40, 6.0, 100, 18).width, 32);
        assert_eq!(tap_area(2, 40.0, 0, 18).x, 0);
    }
}
