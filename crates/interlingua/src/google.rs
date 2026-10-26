//! Google's translation, as CrossLingua Reader asks for it (its "Google V2
//! (Free)" engine): a GET to `translate-pa.googleapis.com/v1/translate`
//! with the key Google's own clients embed. **Not the official Cloud
//! Translation API**: Google may limit or stop it at any time, so this is
//! for one's own use, never for the store.
//!
//! The key is the reader's own, typed in the settings (a secret setting,
//! see [`crate::engine`]); none is in the code.
//!
//! Pure on purpose, as [`crate::text`]: it builds a request and reads an
//! answer, and touches no network. Sending is `Command::http`'s job.

use alloc::format;
use alloc::string::{String, ToString};

use paperpaw_sdk::event::Event;
use paperpaw_sdk::json;
use paperpaw_sdk::replies::HttpReply;
use paperpaw_sdk::web::encode;
use paperpaw_sdk::HttpRequest;

/// Longest text sent at once, in characters: the request carries it in the
/// address.
pub const MAX_CHARS: usize = 1500;

/// The request translating `text` from `from` (`"auto"` to let Google
/// tell) into `to` with `key`; its answer comes as `reply`.
pub fn request(
    text: &str,
    from: &str,
    to: &str,
    key: &str,
    reply: Event<HttpReply>,
) -> HttpRequest {
    let text: String = text.chars().take(MAX_CHARS).collect();
    let url = format!(
        "https://translate-pa.googleapis.com/v1/translate?params.client=gtx\
         &query.source_language={}&query.target_language={}\
         &query.display_language=en-US&data_types=TRANSLATION&key={}&query.text={}",
        encode(from),
        encode(to),
        encode(key.trim()),
        encode(&text),
    );
    HttpRequest::new("GET", &url, reply).header(
        String::from("User-Agent"),
        String::from("PaperPaw/0.1 (e-reader)"),
    )
}

/// The translation in Google's answer
/// (`{"translation":"…","detectedLanguage":"…"}`).
pub fn translation(body: &str) -> Option<String> {
    let members = json::object(body);
    json::text(&members, "translation").map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWERED: Event<HttpReply> = Event::new("interlingua.answered");

    #[test]
    fn reads_the_translation() {
        let body = r#"{"translation":"Alice non aveva nulla da fare.","detectedLanguage":"en"}"#;
        assert_eq!(
            translation(body).as_deref(),
            Some("Alice non aveva nulla da fare.")
        );
    }

    #[test]
    fn reads_an_answer_as_google_really_sends_it() {
        let body = r#"{
  "translation": "Alice era stanca.",
  "detectedLanguages": {
    "srclangs": ["en"],
    "srclangsConfidences": [1],
    "extendedSrclangs": ["en"]
  },
  "sourceLanguage": "en"
}"#;
        assert_eq!(translation(body).as_deref(), Some("Alice era stanca."));
    }

    #[test]
    fn reads_escapes_and_accents() {
        let body = r#"{"translation":"Disse: \"sì\"\n"}"#;
        assert_eq!(translation(body).as_deref(), Some("Disse: \"sì\"\n"));
    }

    #[test]
    fn an_answer_without_it_is_none() {
        assert_eq!(translation(r#"{"error":{"code":400}}"#), None);
        assert_eq!(translation("not json"), None);
    }

    #[test]
    fn the_request_asks_for_the_text_encoded() {
        let req = request("Hi there & you", "en", "it", "K", ANSWERED);
        assert_eq!(req.method(), "GET");
        assert!(req.url().contains("query.source_language=en&query.target_language=it"));
        assert!(req.url().ends_with("&query.text=Hi+there+%26+you"));
    }

    #[test]
    fn the_key_is_trimmed_and_sent() {
        let req = request("Hi", "en", "it", " AIza-key\n", ANSWERED);
        assert!(req.url().contains("&key=AIza-key&query.text=Hi"));
    }

    #[test]
    fn a_long_text_is_cut() {
        let long = "a".repeat(MAX_CHARS + 50);
        let req = request(&long, "en", "it", "K", ANSWERED);
        let sent = req.url().rsplit("&query.text=").next().unwrap_or_default();
        assert_eq!(sent.len(), MAX_CHARS);
    }
}
