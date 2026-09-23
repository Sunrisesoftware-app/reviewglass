//! LocalBackend (adr.rg.023): the OpenAI-compatible chat completions protocol on a
//! loopback host - what Ollama, LM Studio and llama.cpp's server all answer. Loopback is
//! not egress; nothing leaves the machine. An endpoint that does not answer is a normal
//! state (the unit may be off) and says so plainly.

use serde_json::{json, Value};

use super::{client, transport, Failure, Preview, Prompt, Reply};

/// Enough for a few paragraphs from a local model without waiting on a long ramble.
const MAX_TOKENS: u32 = 2048;

pub struct LocalBackend {
    host: String,
    port: u16,
    model: String,
}

impl LocalBackend {
    pub fn new(host: &str, port: u16, model: &str) -> Self {
        Self {
            host: host
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .into(),
            port,
            model: model.into(),
        }
    }

    fn origin(&self) -> String {
        if self.host.contains(':') {
            format!("http://[{}]:{}", self.host, self.port)
        } else {
            format!("http://{}:{}", self.host, self.port)
        }
    }

    pub fn label(&self) -> String {
        format!(
            "Local · {} on {}:{} — stays on this machine",
            self.model, self.host, self.port
        )
    }

    fn body(&self, p: &Prompt) -> Value {
        json!({
            "model": self.model,
            "stream": false,
            "max_tokens": MAX_TOKENS,
            "messages": [
                { "role": "system", "content": p.system },
                { "role": "user", "content": p.user },
            ],
        })
    }

    fn unreachable(&self) -> Failure {
        Failure::Unavailable(format!(
            "nothing answered at {}:{}. Is the local model server running?",
            self.host, self.port
        ))
    }

    pub fn preview(&self, p: &Prompt) -> Preview {
        Preview {
            method: "POST".into(),
            url: format!("{}/v1/chat/completions", self.origin()),
            headers: vec![("content-type".into(), "application/json".into())],
            body: serde_json::to_string_pretty(&self.body(p)).unwrap_or_default(),
        }
    }

    pub async fn explain(&self, p: &Prompt) -> Result<Reply, Failure> {
        let resp = client()?
            .post(format!("{}/v1/chat/completions", self.origin()))
            .header("content-type", "application/json")
            .body(self.body(p).to_string())
            .send()
            .await
            .map_err(|e| transport(e, || self.unreachable()))?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|e| transport(e, || self.unreachable()))?;
        if status != 200 {
            return Err(Failure::Api {
                status,
                message: error_message(&text),
            });
        }
        parse_reply(&text)
    }

    /// The models the endpoint lists (`GET /v1/models`); sends no prompt.
    pub async fn models(&self) -> Result<Vec<String>, Failure> {
        let resp = client()?
            .get(format!("{}/v1/models", self.origin()))
            .send()
            .await
            .map_err(|e| transport(e, || self.unreachable()))?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|e| transport(e, || self.unreachable()))?;
        if status != 200 {
            return Err(Failure::Api {
                status,
                message: error_message(&text),
            });
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| Failure::Parse(e.to_string()))?;
        Ok(v.get("data")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|m| m.get("id").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }
}

fn error_message(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("error"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(200).collect())
}

fn parse_reply(body: &str) -> Result<Reply, Failure> {
    let v: Value = serde_json::from_str(body).map_err(|e| Failure::Parse(e.to_string()))?;
    let choice = v
        .pointer("/choices/0")
        .ok_or_else(|| Failure::Parse("the answer held no choice".into()))?;
    let text = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(Failure::Parse("the answer held no text".into()));
    }
    Ok(Reply::Text {
        text,
        truncated: choice.get("finish_reason").and_then(Value::as_str) == Some("length"),
    })
}

#[cfg(test)]
mod tests {
    use super::super::testserver::serve_once;
    use super::*;

    fn prompt() -> Prompt {
        Prompt {
            system: "S".into(),
            user: "U".into(),
        }
    }

    #[test]
    fn the_chat_request_goes_to_loopback_and_the_text_comes_back() {
        let (port, seen) = serve_once(
            200,
            &[],
            r#"{"choices":[{"message":{"role":"assistant","content":" Adds a print. "},"finish_reason":"stop"}]}"#,
        );
        let b = LocalBackend::new("127.0.0.1", port, "llama3.1");
        let r = tauri::async_runtime::block_on(b.explain(&prompt())).unwrap();
        assert_eq!(
            r,
            Reply::Text {
                text: "Adds a print.".into(),
                truncated: false
            }
        );
        let s = seen.recv().unwrap();
        assert_eq!(s.request_line, "POST /v1/chat/completions HTTP/1.1");
        let body: Value = serde_json::from_str(&s.body).unwrap();
        assert_eq!(body["model"], "llama3.1");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "U");
        let shown: Value = serde_json::from_str(&b.preview(&prompt()).body).unwrap();
        assert_eq!(shown, body);
    }

    #[test]
    fn a_silent_endpoint_is_unavailable_not_an_error_dialog() {
        // A port nothing listens on: bind, note the port, let it go.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let b = LocalBackend::new("127.0.0.1", port, "m");
        assert!(matches!(
            tauri::async_runtime::block_on(b.explain(&prompt())),
            Err(Failure::Unavailable(_))
        ));
    }

    #[test]
    fn the_model_list_and_a_cut_off_answer() {
        let (port, seen) = serve_once(
            200,
            &[],
            r#"{"object":"list","data":[{"id":"llama3.1"},{"id":"qwen2.5-coder"}]}"#,
        );
        let b = LocalBackend::new("localhost", port, "");
        assert_eq!(
            tauri::async_runtime::block_on(b.models()).unwrap(),
            vec!["llama3.1".to_string(), "qwen2.5-coder".to_string()]
        );
        assert_eq!(seen.recv().unwrap().request_line, "GET /v1/models HTTP/1.1");
        assert_eq!(
            parse_reply(r#"{"choices":[{"message":{"content":"Half"},"finish_reason":"length"}]}"#),
            Ok(Reply::Text {
                text: "Half".into(),
                truncated: true
            })
        );
        assert_eq!(
            LocalBackend::new("::1", 8080, "m").preview(&prompt()).url,
            "http://[::1]:8080/v1/chat/completions"
        );
    }
}
