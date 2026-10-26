//! Which service translates: the reader chooses in the settings, and each
//! service has its key there, kept with the passwords and keys (a secret
//! setting is kept under its own id). No key is in the code.

use alloc::string::String;

use paperpaw_sdk::event::Event;
use paperpaw_sdk::replies::HttpReply;
use paperpaw_sdk::{Env, HttpRequest};

use crate::{deepl, google};

/// The service's setting values, and its keys' settings (also the ids of
/// the secrets).
pub const GOOGLE: &str = "interlingua.engine.google";
pub const DEEPL: &str = "interlingua.engine.deepl";
pub const GOOGLE_KEY: &str = "interlingua.google-key";
pub const DEEPL_KEY: &str = "interlingua.deepl-key";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Google,
    DeepL,
}

impl Engine {
    /// The service the setting `value` names (Google when unknown).
    pub fn named(value: &str) -> Self {
        match value {
            DEEPL => Engine::DeepL,
            _ => Engine::Google,
        }
    }

    /// The id of the secret keeping this service's key.
    pub fn key_id(self) -> &'static str {
        match self {
            Engine::Google => GOOGLE_KEY,
            Engine::DeepL => DEEPL_KEY,
        }
    }
}

/// The service chosen.
pub fn chosen(env: &Env<'_>) -> Engine {
    Engine::named(env.settings().read(&crate::ENGINE))
}

/// The key of the service chosen, if the reader gave one.
pub fn key(env: &Env<'_>) -> Option<String> {
    let key = env.secrets().get(chosen(env).key_id())?.trim();
    (!key.is_empty()).then(|| String::from(key))
}

/// Whether translating can start: the chosen service has its key.
pub fn ready(env: &Env<'_>) -> bool {
    key(env).is_some()
}

/// The request translating `text` into `to` with the service chosen;
/// `None` without its key.
pub fn request(
    env: &Env<'_>,
    text: &str,
    to: &str,
    reply: Event<HttpReply>,
) -> Option<HttpRequest> {
    let key = key(env)?;
    Some(match chosen(env) {
        Engine::Google => google::request(text, "auto", to, &key, reply),
        Engine::DeepL => deepl::request(text, to, &key, reply),
    })
}

/// The translation in the chosen service's answer.
pub fn translation(env: &Env<'_>, body: &str) -> Option<String> {
    match chosen(env) {
        Engine::Google => google::translation(body),
        Engine::DeepL => deepl::translation(body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_services() {
        assert_eq!(Engine::named(DEEPL), Engine::DeepL);
        assert_eq!(Engine::named(GOOGLE), Engine::Google);
        assert_eq!(Engine::named("whatever"), Engine::Google);
    }

    #[test]
    fn each_service_has_its_own_key() {
        assert_ne!(Engine::Google.key_id(), Engine::DeepL.key_id());
    }
}
