//! The whole translation of a line's sentences, on a panel over the page:
//! what a tap on the room above a line opens, for a translation that did
//! not fit there. Back, Select or a tap closes it.

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use paperpaw_sdk::builtin::{actions, params};
use paperpaw_sdk::ui::{self, UiStyle};
use paperpaw_sdk::{
    Canvas, Color, Command, Cx, Env, Gesture, GestureKind, Handled, Id, Params, Rect, Screen,
};

/// The screen's id.
pub const SCREEN: Id = Id::new_static("interlingua.full");

/// The panel.
pub struct Full {
    style: UiStyle,
    region: Rect,
    text: String,
    /// The text's lines, as ranges of it, wrapped once.
    lines: Vec<Range<usize>>,
}

impl Full {
    /// The panel for the text in `params` (`params::TEXT`).
    pub fn new(env: &Env<'_>, params: &Params) -> Self {
        let style = UiStyle::new(env);
        let display = env.device().display;
        let text = String::from(params.get(params::TEXT).unwrap_or_default());
        let width = f32::from(display.width.saturating_sub(2 * style.margin()));
        let lines = style
            .font(env)
            .map(|font| font.wrap(style.text(), &text, width))
            .unwrap_or_default();
        // As tall as its lines, below a heading, at most half the screen.
        let line = style.row();
        let rows = (lines.len() as u16).clamp(1, (display.height / 2) / line);
        let height = line * (rows + 1) + line / 2;
        Self {
            style,
            region: Rect::new(0, display.height - height, display.width, height),
            text,
            lines,
        }
    }
}

impl Screen for Full {
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
        let margin = f32::from(style.margin());
        let row = style.row();
        ui::Label::new(env.i18n().tr("interlingua.full.title"))
            .px(style.small())
            .color(style.faint(ui::Ink::Detail))
            .draw_from(&mut ui, margin, Rect::new(r.x, r.y, r.width, row));
        let Some(font) = ui.font() else {
            return;
        };
        let ascent = font.line_metrics(style.text()).ascent;
        let rows = usize::from((r.height - row) / row);
        for (n, range) in self.lines.iter().take(rows).enumerate() {
            let top = f32::from(r.y + row) + f32::from(row) * n as f32;
            let baseline = (top + (f32::from(row) - ascent) / 2.0 + ascent) as i32;
            font.draw(
                ui.canvas(),
                style.text(),
                margin,
                baseline,
                &self.text[range.clone()],
                Color::BLACK,
            );
        }
    }

    fn on_action(&mut self, action: &Id, cx: &mut Cx<'_, '_>) -> Handled {
        if *action != actions::BACK && *action != actions::CONFIRM {
            return Handled::No;
        }
        cx.command(Command::Back);
        Handled::Yes
    }

    fn on_gesture(&mut self, gesture: Gesture, cx: &mut Cx<'_, '_>) -> Handled {
        if gesture.kind != GestureKind::Tap {
            return Handled::No;
        }
        cx.command(Command::Back);
        Handled::Yes
    }
}
