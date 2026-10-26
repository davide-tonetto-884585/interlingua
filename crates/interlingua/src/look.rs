//! How the translation looks: its tone, as a colour, and how a small size
//! changes it. Pure, tested with `cargo test`.

use paperpaw_sdk::Color;

/// The setting's value for a tone (`interlingua.shade.<name>`): a gray
/// (`0`, `80`, `130`: its value, 0 black) or a colour by name. All are dark
/// enough to read on a panel of few grays, where a colour is drawn as its
/// luminance.
pub fn tone(key: &str) -> Color {
    match key.rsplit('.').next().unwrap_or("") {
        "blue" => Color::rgb(0, 60, 150),
        "red" => Color::rgb(160, 30, 30),
        "green" => Color::rgb(20, 110, 50),
        "brown" => Color::rgb(120, 70, 20),
        "purple" => Color::rgb(90, 40, 130),
        gray => Color::gray(gray.parse().unwrap_or(80)),
    }
}

/// Size in pixels from which a tone is drawn as chosen.
const FULL_SIZE: f32 = 14.0;

/// `tone` for text of `size` pixels, darker the smaller the text: thin
/// strokes in a light tone fade away on a panel of few grays (a light
/// gray at 9 pixels shows nothing on an X4).
pub fn readable(tone: Color, size: f32) -> Color {
    let scale = (size / FULL_SIZE).clamp(0.0, 1.0);
    let darker = |channel: u8| (f32::from(channel) * scale) as u8;
    Color::rgb(darker(tone.r), darker(tone.g), darker(tone.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grays_and_colours_come_from_their_names() {
        assert_eq!(tone("interlingua.shade.0"), Color::BLACK);
        assert_eq!(tone("interlingua.shade.130"), Color::gray(130));
        assert_eq!(tone("interlingua.shade.blue"), Color::rgb(0, 60, 150));
        assert_eq!(tone("interlingua.shade.nonsense"), Color::gray(80));
    }

    #[test]
    fn a_small_size_darkens_the_tone_a_big_one_leaves_it() {
        assert_eq!(readable(Color::gray(130), 14.0), Color::gray(130));
        assert_eq!(readable(Color::gray(130), 28.0), Color::gray(130));
        assert_eq!(readable(Color::gray(130), 7.0), Color::gray(65));
        assert_eq!(readable(Color::rgb(160, 30, 30), 7.0), Color::rgb(80, 15, 15));
    }

    #[test]
    fn every_tone_is_dark_enough_to_read() {
        for name in ["0", "80", "130", "blue", "red", "green", "brown", "purple"] {
            assert!(tone(name).luma() <= 130, "{name}");
        }
    }
}
