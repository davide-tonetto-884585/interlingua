//! DeepL's translation, the official API: a POST to `/v2/translate` with
//! the reader's own key. Keys of the free plan end in `:fx` and go to
//! DeepL's free address.
//!
//! Pure as [`crate::google`]: it builds a request and reads an answer.

use alloc::format;
use alloc::string::{String, ToString};

use paperpaw_sdk::event::Event;
use paperpaw_sdk::json;
use paperpaw_sdk::replies::HttpReply;
use paperpaw_sdk::HttpRequest;

/// Longest text sent at once, in characters.
pub const MAX_CHARS: usize = 4000;

/// DeepL's code for the target `to` (Google's, lowercase): it wants
/// capitals, and a region for some languages.
fn language(to: &str) -> String {
    match to {
        "en" => String::from("EN-US"),
        "pt" => String::from("PT-PT"),
        "zh-CN" => String::from("ZH"),
        other => other.to_uppercase(),
    }
}

/// The request translating `text` into `to` with `key`; its answer comes
/// as `reply`.
pub fn request(text: &str, to: &str, key: &str, reply: Event<HttpReply>) -> HttpRequest {
    let key = key.trim();
    let host = match key.ends_with(":fx") {
        true => "api-free.deepl.com",
        false => "api.deepl.com",
    };
    let text: String = text.chars().take(MAX_CHARS).collect();
    let body = format!(
        "{{\"text\":[{}],\"target_lang\":{}}}",
        json::string(&text),
        json::string(&language(to)),
    );
    HttpRequest::new("POST", &format!("https://{host}/v2/translate"), reply)
        .header(String::from("Authorization"), format!("DeepL-Auth-Key {key}"))
        .header(String::from("Content-Type"), String::from("application/json"))
        .body(body)
}

/// The translation in DeepL's answer
/// (`{"translations":[{"detected_source_language":"EN","text":"…"}]}`).
pub fn translation(body: &str) -> Option<String> {
    let list = json::raw_member(body, "translations")?;
    let first = list.trim().strip_prefix('[')?.trim_start();
    let end = first.rfind('}')?;
    let members = json::object(&first[..=end]);
    json::text(&members, "text").map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWERED: Event<HttpReply> = Event::new("interlingua.answered");

    #[test]
    fn reads_the_translation() {
        let body = r#"{"translations":[{"detected_source_language":"EN","text":"Alice era stanca."}]}"#;
        assert_eq!(translation(body).as_deref(), Some("Alice era stanca."));
    }

    #[test]
    fn reads_escapes_and_whitespace() {
        let body = "{\n \"translations\": [\n  {\"text\": \"Disse: \\\"sì\\\"\"}\n ]\n}";
        assert_eq!(translation(body).as_deref(), Some("Disse: \"sì\""));
    }

    #[test]
    fn an_answer_without_it_is_none() {
        assert_eq!(translation(r#"{"message":"Forbidden"}"#), None);
        assert_eq!(translation(r#"{"translations":[]}"#), None);
        assert_eq!(translation("not json"), None);
    }

    #[test]
    fn the_request_is_a_post_with_the_key_and_the_text_as_json() {
        let req = request("Hi \"you\"", "it", "abc", ANSWERED);
        assert_eq!(req.method(), "POST");
        assert_eq!(req.url(), "https://api.deepl.com/v2/translate");
        assert_eq!(
            req.body_text(),
            Some(r#"{"text":["Hi \"you\""],"target_lang":"IT"}"#)
        );
        assert!(req
            .headers()
            .iter()
            .any(|(n, v)| n == "Authorization" && v == "DeepL-Auth-Key abc"));
    }

    #[test]
    fn a_free_key_goes_to_the_free_address() {
        let req = request("Hi", "it", " abc:fx\n", ANSWERED);
        assert_eq!(req.url(), "https://api-free.deepl.com/v2/translate");
    }

    #[test]
    fn languages_get_deepls_codes() {
        assert_eq!(language("en"), "EN-US");
        assert_eq!(language("pt"), "PT-PT");
        assert_eq!(language("zh-CN"), "ZH");
        assert_eq!(language("de"), "DE");
    }
}
