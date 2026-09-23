//! RemoteBackend (adr.rg.023): the Claude Messages API, with the user's own key.
//!
//! Raw HTTP, since Rust has no official SDK: `POST /v1/messages` with `x-api-key`,
//! `anthropic-version: 2023-06-01`, and the server-side refusal fallback
//! (`fallbacks: "default"`, beta `server-side-fallback-2026-07-01`), so a request the
//! model declines is re-run on the model Anthropic recommends for that category rather
//! than coming back empty. `stop_reason` is read before `content`: a refusal is an
//! answer, not an error. No retries.

use serde_json::{json, Value};

use super::{client, transport, Failure, Preview, Prompt, Reply};

pub const DEFAULT_MODEL: &str = "claude-opus-5";
const API: &str = "https://api.anthropic.com";
const VERSION: &str = "2023-06-01";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Room for the answer; a non-streaming request keeps under the HTTP time limits at this.
const MAX_TOKENS: u32 = 16000;
/// An explanation of one hunk is routine work: medium effort, not the default high.
const EFFORT: &str = "medium";

pub struct RemoteBackend {
    base: String,
    model: String,
    key: Option<String>,
}

impl RemoteBackend {
    pub fn new(model: &str, key: Option<String>) -> Self {
        Self {
            base: API.into(),
            model: model.into(),
            key,
        }
    }

    #[cfg(test)]
    fn at(base: &str, model: &str, key: Option<&str>) -> Self {
        Self {
            base: base.into(),
            model: model.into(),
            key: key.map(str::to_string),
        }
    }

    pub fn ready(&self) -> Result<(), Failure> {
        if self.key.as_deref().is_some_and(|k| !k.trim().is_empty()) {
            Ok(())
        } else {
            Err(Failure::NoKey)
        }
    }

    pub fn label(&self) -> String {
        format!("Remote · Claude API · {} — leaves this machine", self.model)
    }

    fn url(&self) -> String {
        format!("{}/v1/messages", self.base)
    }

    fn body(&self, p: &Prompt) -> Value {
        json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "fallbacks": "default",
            "output_config": { "effort": EFFORT },
            "system": p.system,
            "messages": [ { "role": "user", "content": p.user } ],
        })
    }

    pub fn preview(&self, p: &Prompt) -> Preview {
        Preview {
            method: "POST".into(),
            url: self.url(),
            headers: vec![
                ("content-type".into(), "application/json".into()),
                ("anthropic-version".into(), VERSION.into()),
                ("anthropic-beta".into(), FALLBACK_BETA.into()),
                (
                    "x-api-key".into(),
                    "(your key, read from Windows Credential Manager — not shown)".into(),
                ),
            ],
            body: serde_json::to_string_pretty(&self.body(p)).unwrap_or_default(),
        }
    }

    pub async fn explain(&self, p: &Prompt) -> Result<Reply, Failure> {
        let Some(key) = self.key.as_deref().filter(|k| !k.trim().is_empty()) else {
            return Err(Failure::NoKey);
        };
        let resp = client()?
            .post(self.url())
            .header("content-type", "application/json")
            .header("anthropic-version", VERSION)
            .header("anthropic-beta", FALLBACK_BETA)
            .header("x-api-key", key.trim())
            .body(self.body(p).to_string())
            .send()
            .await
            .map_err(|e| {
                transport(e, || {
                    Failure::Network("the Claude API could not be reached".into())
                })
            })?;
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let text = resp
            .text()
            .await
            .map_err(|e| transport(e, || Failure::Network("the answer was cut off".into())))?;
        if status == 200 {
            parse_reply(&text)
        } else {
            Err(failure(status, &text, retry_after.as_deref()))
        }
    }
}

/// A 200's body: the refusal first, then the text blocks (thinking blocks, which come
/// back empty by default, and fallback markers are skipped).
fn parse_reply(body: &str) -> Result<Reply, Failure> {
    let v: Value = serde_json::from_str(body).map_err(|e| Failure::Parse(e.to_string()))?;
    let stop = v.get("stop_reason").and_then(Value::as_str).unwrap_or("");
    if stop == "refusal" {
        let why = v
            .pointer("/stop_details/explanation")
            .and_then(Value::as_str)
            .unwrap_or("the model declined to explain this change");
        return Ok(Reply::Refused(why.to_string()));
    }
    let text: Vec<&str> = v
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let text = text.join("\n\n").trim().to_string();
    if text.is_empty() {
        return Err(Failure::Parse("the answer held no text".into()));
    }
    Ok(Reply::Text {
        text,
        truncated: stop == "max_tokens",
    })
}

/// A non-200 in the user's terms, with the service's own message where it gave one.
fn failure(status: u16, body: &str, retry_after: Option<&str>) -> Failure {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(200).collect());
    match status {
        401 | 403 => Failure::Auth(message),
        429 => Failure::RateLimited(match retry_after {
            Some(s) => format!("Try again in {s} s."),
            None => "Try again in a moment.".into(),
        }),
        500 | 502 | 503 | 529 => Failure::Overloaded(message),
        404 => Failure::Api {
            status,
            message: format!("{message} (is the model name in Settings right?)"),
        },
        _ => Failure::Api { status, message },
    }
}

#[cfg(test)]
mod tests {
    use super::super::testserver::serve_once;
    use super::*;

    fn prompt() -> Prompt {
        Prompt {
            system: "Explain for a novice.".into(),
            user: "File: a.py\n```diff\n+print(1)\n```".into(),
        }
    }

    #[test]
    fn the_request_carries_the_key_the_version_and_the_fallback_and_the_text_comes_back() {
        let (port, seen) = serve_once(
            200,
            &[],
            r#"{"type":"message","stop_reason":"end_turn","content":[{"type":"thinking","thinking":""},{"type":"text","text":"It prints one."}]}"#,
        );
        let b = RemoteBackend::at(&format!("http://127.0.0.1:{port}"), "m-1", Some("sk-test"));
        let r = tauri::async_runtime::block_on(b.explain(&prompt())).unwrap();
        assert_eq!(
            r,
            Reply::Text {
                text: "It prints one.".into(),
                truncated: false
            }
        );
        let s = seen.recv().unwrap();
        assert_eq!(s.request_line, "POST /v1/messages HTTP/1.1");
        assert_eq!(s.header("x-api-key"), Some("sk-test"));
        assert_eq!(s.header("anthropic-version"), Some(VERSION));
        assert_eq!(s.header("anthropic-beta"), Some(FALLBACK_BETA));
        let body: Value = serde_json::from_str(&s.body).unwrap();
        assert_eq!(body["model"], "m-1");
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["system"], "Explain for a novice.");
        assert_eq!(body["messages"][0]["role"], "user");
        // What was sent is exactly what the disclosure shows.
        let shown: Value = serde_json::from_str(&b.preview(&prompt()).body).unwrap();
        assert_eq!(shown, body);
        assert!(!b
            .preview(&prompt())
            .headers
            .iter()
            .any(|(_, v)| v.contains("sk-test")));
    }

    #[test]
    fn no_key_sends_nothing() {
        let b = RemoteBackend::at("http://127.0.0.1:9", "m", None);
        assert_eq!(b.ready(), Err(Failure::NoKey));
        assert_eq!(
            RemoteBackend::at("x", "m", Some(" ")).ready(),
            Err(Failure::NoKey)
        );
        assert_eq!(RemoteBackend::at("x", "m", Some("k")).ready(), Ok(()));
        assert_eq!(
            tauri::async_runtime::block_on(b.explain(&prompt())),
            Err(Failure::NoKey)
        );
    }

    #[test]
    fn a_refusal_is_an_answer_and_a_cut_off_answer_says_so() {
        assert_eq!(
            parse_reply(
                r#"{"stop_reason":"refusal","stop_details":{"type":"refusal","category":null,"explanation":"declined"},"content":[]}"#
            ),
            Ok(Reply::Refused("declined".into()))
        );
        assert_eq!(
            parse_reply(
                r#"{"stop_reason":"max_tokens","content":[{"type":"text","text":"Half"}]}"#
            ),
            Ok(Reply::Text {
                text: "Half".into(),
                truncated: true
            })
        );
        assert!(matches!(
            parse_reply(r#"{"stop_reason":"end_turn","content":[]}"#),
            Err(Failure::Parse(_))
        ));
    }

    #[test]
    fn errors_are_named_by_what_the_user_can_do() {
        let body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        assert_eq!(
            failure(401, body, None),
            Failure::Auth("invalid x-api-key".into())
        );
        assert_eq!(
            failure(429, "{}", Some("12")),
            Failure::RateLimited("Try again in 12 s.".into())
        );
        assert!(
            matches!(failure(529, r#"{"error":{"message":"Overloaded"}}"#, None), Failure::Overloaded(m) if m == "Overloaded")
        );
        assert!(matches!(
            failure(404, "{}", None),
            Failure::Api { status: 404, .. }
        ));
        let (port, _seen) = serve_once(401, &[], body);
        let b = RemoteBackend::at(&format!("http://127.0.0.1:{port}"), "m", Some("bad"));
        assert_eq!(
            tauri::async_runtime::block_on(b.explain(&prompt())),
            Err(Failure::Auth("invalid x-api-key".into()))
        );
    }
}
