//! The panel shown while a chapter is translated: it turns the Wi-Fi on,
//! asks for a paragraph at a time, keeps each answer on the card and says
//! how far it has come. Back stops it.
//!
//! The requests and their answers are the screen's, not the plugin's: a
//! screen is where the network is asked for and where it can be waited
//! for (PaperPaw keeps the network for the screen that went online until
//! it closes), and where the reader sees what is going on.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::cell::RefCell;

use paperpaw_sdk::builtin::{actions, params};
use paperpaw_sdk::document::{Document, DocumentError};
use paperpaw_sdk::event::Event;
use paperpaw_sdk::replies::HttpReply;
use paperpaw_sdk::SettingValue;
use paperpaw_sdk::ui::{self, UiStyle};
use paperpaw_sdk::wifi::{WifiCommand, WifiState};
use paperpaw_sdk::work::Reason;
use paperpaw_sdk::{
    Canvas, Color, Command, Cx, Env, Handled, Id, Instant, Params, Rect, Screen, read_file,
};

use crate::chapter::{self, Job, Next};
use crate::{cache, engine};

/// The screen's id.
pub const SCREEN: Id = Id::new_static("interlingua.progress");
/// The parameter with the offset where the job stops (`params::OFFSET`
/// is where it starts).
pub const UNTIL: &str = "until";
/// The answer to a paragraph's translation request.
pub const ANSWERED: Event<HttpReply> = Event::new("interlingua.answered");
const ANSWERED_ID: Id = Id::new_static("interlingua.answered");
/// Largest answer read back: one paragraph's translation is far below.
const MOST_ANSWER: usize = 16 * 1024;
/// Milliseconds between an answer and the next request: the panel is
/// drawn in between.
const STEP_PAUSE_MS: u64 = 30;


/// Where the job is, for the words on the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Waiting for the network: the first answer is not here yet.
    Connecting,
    Translating,
    /// Ended; the key of what to say.
    Ended(&'static str),
}

/// The panel and the job behind it.
pub struct Progress {
    style: UiStyle,
    region: Rect,
    job: Job,
    stage: Stage,
    /// Paragraphs translated and kept.
    stored: u32,
    /// How far the chapter is, in thousandths.
    permille: u16,
    /// The radio was off when the job began: it is switched off again.
    wifi_was_off: bool,
    /// The next paragraph is to be asked for at the next timer: the panel
    /// is drawn first.
    advance_due: bool,
    /// What the last answer was, as numbers, when it was not a translation:
    /// shown under the message so that a failure can be told apart.
    detail: String,
}

impl Progress {
    /// The panel for the section of the book that `params` name.
    pub fn new(env: &Env<'_>, params: &Params) -> Self {
        let display = env.device().display;
        let style = UiStyle::new(env);
        let height = style.row() * 4;
        let top = display.height.saturating_sub(height);
        let section = params
            .get(params::SECTION)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        Self {
            style,
            region: Rect::new(0, top, display.width, height),
            job: Job::range(
                params.get(params::PATH).unwrap_or_default(),
                section,
                number(params, params::OFFSET).unwrap_or(0),
                number(params, UNTIL).unwrap_or(u32::MAX),
            ),
            stage: Stage::Connecting,
            stored: 0,
            permille: 0,
            wifi_was_off: false,
            advance_due: false,
            detail: String::new(),
        }
    }

    /// Sends the request for the job's next paragraph, or ends the job.
    fn advance(&mut self, env: &Env<'_>, cx: &mut Cx<'_, '_>) {
        let storage = env.storage();
        // What is kept is looked up in the file of the paragraph's stretch,
        // read when the job gets to it (one at a time, as the paragraphs
        // come in order).
        let stretch = RefCell::new((String::new(), String::new()));
        let (path, section) = (self.job.path.clone(), self.job.section);
        let kept = |offset: u32| {
            let file = crate::cache_file(env, &path, section, offset);
            let mut stretch = stretch.borrow_mut();
            if stretch.0 != file {
                let data = cache::load(storage, &file).unwrap_or_default();
                *stretch = (file, data);
            }
            cache::lookup(&stretch.1, offset).is_some()
        };
        // The document is opened for this paragraph only and dropped at
        // the end of the statement.
        let job = &mut self.job;
        let step = open(env, &job.path).and_then(|document| {
            let length = document.section_len(job.section);
            chapter::next(&*document, storage, job, &kept).map(|next| (next, length))
        });
        match step {
            Ok((Next::Paragraph { text, .. }, length)) => {
                let done = u64::from(self.job.offset) * 1000 / u64::from(length.max(1));
                self.permille = done.min(1000) as u16;
                let to = crate::target(env);
                match engine::request(env, &text, &to, ANSWERED) {
                    Some(request) => cx.command(Command::http(request.save_to(answer_file()))),
                    None => self.end(env, "interlingua.no-key", cx),
                }
            }
            Ok((Next::Finished { more }, _)) => {
                self.permille = 1000;
                self.end(env, if more { "interlingua.more" } else { "interlingua.done" }, cx);
            }
            Err(_) => self.end(env, "interlingua.failed", cx),
        }
    }

    /// Ends the job: pages are laid out again with what was kept, the
    /// radio goes back off if the job switched it on, and the reader is
    /// told how it went (a chapter done closes the panel).
    fn end(&mut self, env: &Env<'_>, message: &'static str, cx: &mut Cx<'_, '_>) {
        if matches!(self.stage, Stage::Ended(_)) {
            return;
        }
        self.stage = Stage::Ended(message);
        if self.stored > 0 {
            // A setting changed: the reader asks the filters again, and
            // lays the page out anew with what was kept.
            let stamp = env.settings().read(&crate::STAMP);
            let next = SettingValue::Int(stamp.wrapping_add(1) % crate::MOST_STAMP);
            cx.command(Command::SetSetting(Id::new_static(crate::STAMP.static_str()), next));
        }
        if self.wifi_was_off {
            cx.command(Command::Wifi(WifiCommand::Off));
        }
        if matches!(message, "interlingua.done" | "interlingua.more") {
            cx.command(Command::Notify(env.i18n().tr(message).into()));
            cx.command(Command::Back);
        }
    }

    /// The panel's words: what is going on.
    fn status(&self, env: &Env<'_>) -> String {
        let i18n = env.i18n();
        match self.stage {
            Stage::Connecting => String::from(i18n.tr("interlingua.progress.connecting")),
            Stage::Translating => format!(
                "{} {} ({} %)",
                i18n.tr("interlingua.progress.translated"),
                self.stored,
                self.permille / 10
            ),
            Stage::Ended(message) => String::from(i18n.tr(message)),
        }
    }
}

impl Screen for Progress {
    fn region(&self, _: &Env<'_>) -> Option<Rect> {
        Some(self.region)
    }

    fn draw(&mut self, canvas: &mut Canvas<'_>, env: &Env<'_>) {
        let style = self.style;
        let mut ui = ui::Ui::new(canvas, env, &style);
        let r = self.region;
        ui.canvas().fill(r, Color::WHITE);
        ui.canvas().fill_rect(
            i32::from(r.x),
            i32::from(r.y),
            u32::from(r.width),
            (u32::from(style.line()) * 2).max(2),
            Color::BLACK,
        );
        let margin = style.margin();
        let row = style.row();
        let title = Rect::new(r.x, r.y, r.width, row);
        ui::Label::new(env.i18n().tr("interlingua.progress.title"))
            .px(style.title())
            .color(Color::BLACK)
            .draw_from(&mut ui, f32::from(margin), title);
        let status = Rect::new(r.x, r.y + row, r.width, row);
        ui::Label::new(&self.status(env))
            .px(style.text())
            .color(Color::BLACK)
            .draw_from(&mut ui, f32::from(margin), status);
        let bar = Rect::new(
            r.x + margin,
            r.y + 2 * row + row / 4,
            r.width.saturating_sub(2 * margin),
            row / 2,
        );
        ui.progress(bar, self.permille, Color::BLACK);
        let hint = Rect::new(r.x, r.y + 3 * row, r.width, row);
        let hint_text = match self.detail.is_empty() {
            true => env.i18n().tr("interlingua.progress.back"),
            false => self.detail.as_str(),
        };
        ui::Label::new(hint_text)
            .px(style.small())
            .color(style.faint(ui::Ink::Detail))
            .draw_from(&mut ui, f32::from(margin), hint);
    }

    fn on_show(&mut self, cx: &mut Cx<'_, '_>) {
        let env = *cx.env();
        if let Some(wifi) = env.wifi() {
            self.wifi_was_off = matches!(wifi.state, WifiState::Off);
            if !matches!(wifi.state, WifiState::Online { .. } | WifiState::Joining { .. }) {
                // Offline mode asks to go online for this screen; the
                // radio, if it is off, is switched on.
                cx.command(Command::GoOnline);
                if self.wifi_was_off {
                    cx.command(Command::Wifi(WifiCommand::On));
                }
            }
        } else if !env.caps().network {
            return self.end(&env, "interlingua.no-network", cx);
        }
        self.advance(&env, cx);
    }

    fn on_event(&mut self, event: &Id, params: &Params, cx: &mut Cx<'_, '_>) {
        let Some(reply) = ANSWERED.read(event, params) else {
            return;
        };
        let env = *cx.env();
        if matches!(self.stage, Stage::Ended(_)) {
            return;
        }
        self.stage = Stage::Translating;
        let waiting = self.job.waiting.take();
        let body = answer_text(&env, &reply);
        let kept = match (reply.status, waiting) {
            (200..=299, Some(offset)) => {
                engine::translation(&env, &body).map(|translation| (offset, translation))
            }
            _ => None,
        };
        let Some((offset, translation)) = kept else {
            self.detail = format!("HTTP {}, {} B", reply.status, body.len());
            let message = match (&reply.refused, reply.status) {
                (Some(refusal), _) if refusal.reason == Reason::NoNetwork => "interlingua.no-wifi",
                (_, 0) => "interlingua.unreachable",
                _ => "interlingua.failed",
            };
            self.end(&env, message, cx);
            cx.request_redraw();
            return;
        };
        let file = crate::cache_file(&env, &self.job.path, self.job.section, offset);
        match cache::store(env.storage(), &file, offset, &translation) {
            Ok(()) => {
                self.stored += 1;
                // Draw what is done now; the next paragraph is asked for
                // when the timer comes, after the panel was drawn.
                self.advance_due = true;
                cx.wake_at(Instant(env.now().millis() + STEP_PAUSE_MS));
            }
            Err(_) => self.end(&env, "interlingua.failed", cx),
        }
        cx.request_redraw();
    }

    fn on_timer(&mut self, cx: &mut Cx<'_, '_>) {
        if !self.advance_due {
            return;
        }
        self.advance_due = false;
        let env = *cx.env();
        self.advance(&env, cx);
        cx.request_redraw();
    }

    fn on_action(&mut self, action: &Id, cx: &mut Cx<'_, '_>) -> Handled {
        if *action != actions::BACK && *action != actions::CONFIRM {
            return Handled::No;
        }
        let env = *cx.env();
        if !matches!(self.stage, Stage::Ended(_)) {
            cx.command(Command::CancelHttp(ANSWERED_ID));
            self.end(&env, "interlingua.stopped", cx);
        }
        cx.command(Command::Back);
        Handled::Yes
    }
}

/// The number in `params` under `key`, if there is one.
fn number(params: &Params, key: &str) -> Option<u32> {
    params.get(key)?.parse().ok()
}

/// Where the shell writes an answer's body. A body that comes in a reply
/// reaches a plugin only through a value of the reply's own type, which a
/// plugin built apart from the firmware (the emulator's library, a
/// `code.ppn`) may not share: a file is the same for everyone.
fn answer_file() -> String {
    format!("{}/answer.json", Env::plugin_dir("interlingua"))
}

/// An answer's body: the one in the reply where it is there, else the one
/// in [`answer_file`], which is then removed.
fn answer_text(env: &Env<'_>, reply: &HttpReply) -> String {
    if !reply.body.is_empty() {
        return reply.text().into_owned();
    }
    let file = answer_file();
    let text = read_file(env.storage(), &file, MOST_ANSWER)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default();
    let _ = env.storage().remove(&file);
    text
}

/// The book at `path` opened by its format, as the reader opens it.
fn open(env: &Env<'_>, path: &str) -> Result<Box<dyn Document>, DocumentError> {
    let formats = env.formats();
    let format = formats.for_path(path).ok_or(DocumentError::UnsupportedFormat)?;
    (format.open)(env.storage(), path, &formats.options())
}
