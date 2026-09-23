//! Explain backend (rg.explain-backend, adr.rg.002, adr.rg.023): one call in, text or a
//! typed failure out, behind which two implementations stand as equals.
//!
//! - `local`: the OpenAI-compatible chat completions protocol on a loopback host (a local
//!   compute unit: Ollama, LM Studio, llama.cpp's server). Loopback is not egress.
//! - `remote`: the Claude Messages API with the user's own key, kept in Windows Credential
//!   Manager (`credential`), never in a file, a log or the repository.
//!
//! This directory is the only place a provider is named. The explain service hands a
//! provider-agnostic `Prompt` in and gets a `Reply` or a `Failure` back; a third backend is
//! one more module here and one more arm in `Backend`, and nothing outside changes.
//!
//! Both are asynchronous, one request at a time (the service enforces it), time out at
//! `TIMEOUT`, and never retry: a failed explanation is a user-visible non-event. Dropping
//! the future drops the connection, which is what cancellation from the UI does.

pub mod credential;
mod local;
mod remote;

use std::fmt;
use std::time::Duration;

use serde::Serialize;

use crate::explain::{BackendKind, ExplainConfig};

/// A request's time limit. The spec proposed 30 s; the default remote model thinks before
/// it answers, so 60 s (adr.rg.023).
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// What the explain service asks: the instructions and the change. Built by the service,
/// provider-agnostic; a backend only maps it to its wire format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub system: String,
    pub user: String,
}

/// Exactly what a request will send, for the disclosure before the first remote send
/// (spec 6.3). Built by the same code that builds the request.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Preview {
    pub method: String,
    pub url: String,
    /// Name and value; a secret is shown as a placeholder, never its value.
    pub headers: Vec<(String, String)>,
    /// The body, pretty-printed JSON.
    pub body: String,
}

/// A backend's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// The explanation; `truncated` when it hit the output limit.
    Text { text: String, truncated: bool },
    /// The model declined (a normal answer, not an error), with its stated reason.
    Refused(String),
}

/// Why no explanation came. Every variant says so in the user's terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The local endpoint did not answer: a normal state (the unit may be off).
    Unavailable(String),
    /// No key is stored for the remote backend.
    NoKey,
    /// The key was not accepted.
    Auth(String),
    /// Too many requests; the text says when to try again, if the server said.
    RateLimited(String),
    /// The service is overloaded or failing on its side.
    Overloaded(String),
    /// No answer within `TIMEOUT`.
    Timeout,
    /// The network failed before an answer.
    Network(String),
    /// Any other answer that was not an explanation.
    Api { status: u16, message: String },
    /// An answer that could not be read.
    Parse(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Unavailable(m) => write!(f, "Backend unavailable: {m}"),
            Failure::NoKey => write!(
                f,
                "No API key is stored. Add one in Settings > Explain; it is kept in Windows Credential Manager."
            ),
            Failure::Auth(m) => write!(f, "The API key was not accepted: {m}"),
            Failure::RateLimited(m) => write!(f, "Too many requests right now. {m}"),
            Failure::Overloaded(m) => write!(f, "The service is busy or failing on its side: {m}"),
            Failure::Timeout => write!(
                f,
                "No answer within {} s; nothing more will be waited for.",
                TIMEOUT.as_secs()
            ),
            Failure::Network(m) => write!(f, "The network failed before an answer: {m}"),
            Failure::Api { status, message } => write!(f, "HTTP {status}: {message}"),
            Failure::Parse(m) => write!(f, "The answer could not be read: {m}"),
        }
    }
}

/// The configured backend, ready to call.
pub enum Backend {
    Local(local::LocalBackend),
    Remote(remote::RemoteBackend),
}

/// The model the remote backend uses when the setting is empty.
pub fn remote_default_model() -> &'static str {
    remote::DEFAULT_MODEL
}

/// What the remote backend is, in the settings' words.
pub fn remote_service() -> &'static str {
    "the Claude API"
}

/// Local servers known to speak the local backend's protocol, and their usual ports.
pub fn local_servers() -> &'static str {
    "Ollama (port 11434), LM Studio (1234) or llama.cpp's server"
}

/// Whether a host is this machine: the local backend takes nothing else, since loopback
/// traffic is the only kind that is not egress.
pub fn is_loopback(host: &str) -> bool {
    let h = host.trim().trim_start_matches('[').trim_end_matches(']');
    h.eq_ignore_ascii_case("localhost")
        || h.parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

impl Backend {
    /// The backend the settings choose; `None` when explanations are off (the default).
    /// The remote key is read from Credential Manager here, at the moment of use.
    pub fn from_config(cfg: &ExplainConfig) -> Result<Option<Backend>, String> {
        match cfg.backend {
            BackendKind::Off => Ok(None),
            BackendKind::Local => {
                if !is_loopback(&cfg.local_host) {
                    return Err(format!(
                        "The local backend must be on this machine (127.0.0.1, ::1 or localhost), not {}.",
                        cfg.local_host
                    ));
                }
                if cfg.local_model.trim().is_empty() {
                    return Err(
                        "The local backend needs a model name (the one the local server serves)."
                            .into(),
                    );
                }
                Ok(Some(Backend::Local(local::LocalBackend::new(
                    &cfg.local_host,
                    cfg.local_port,
                    cfg.local_model.trim(),
                ))))
            }
            BackendKind::Remote => {
                let model = cfg
                    .remote_model
                    .as_deref()
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                    .unwrap_or(remote::DEFAULT_MODEL);
                Ok(Some(Backend::Remote(remote::RemoteBackend::new(
                    model,
                    credential::read(),
                ))))
            }
        }
    }

    /// The name shown with every request, so local and remote are never confused.
    pub fn label(&self) -> String {
        match self {
            Backend::Local(b) => b.label(),
            Backend::Remote(b) => b.label(),
        }
    }

    /// Whether a request leaves the machine.
    pub fn is_remote(&self) -> bool {
        matches!(self, Backend::Remote(_))
    }

    /// What must be in place before a request is worth preparing: for the remote
    /// backend, a stored key. Checked before the disclosure, so a yes is never asked for
    /// (and never spent) on a request that could not be sent.
    pub fn ready(&self) -> Result<(), Failure> {
        match self {
            Backend::Local(_) => Ok(()),
            Backend::Remote(b) => b.ready(),
        }
    }

    pub fn preview(&self, prompt: &Prompt) -> Preview {
        match self {
            Backend::Local(b) => b.preview(prompt),
            Backend::Remote(b) => b.preview(prompt),
        }
    }

    pub async fn explain(&self, prompt: &Prompt) -> Result<Reply, Failure> {
        match self {
            Backend::Local(b) => b.explain(prompt).await,
            Backend::Remote(b) => b.explain(prompt).await,
        }
    }
}

/// Whether the local endpoint answers, and the models it lists. Sends no prompt.
pub async fn check_local(cfg: &ExplainConfig) -> Result<Vec<String>, Failure> {
    if !is_loopback(&cfg.local_host) {
        return Err(Failure::Unavailable(format!(
            "{} is not this machine; the local backend takes a loopback host only",
            cfg.local_host
        )));
    }
    local::LocalBackend::new(&cfg.local_host, cfg.local_port, cfg.local_model.trim())
        .models()
        .await
}

/// A client with the time limit. `no_proxy`: a loopback request never goes through a
/// proxy, and the remote one goes straight to the service it names.
fn client() -> Result<reqwest::Client, Failure> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .no_proxy()
        .build()
        .map_err(|e| Failure::Network(e.to_string()))
}

/// A transport error in the user's terms.
fn transport(e: reqwest::Error, unreachable: impl FnOnce() -> Failure) -> Failure {
    if e.is_timeout() {
        Failure::Timeout
    } else if e.is_connect() {
        unreachable()
    } else {
        Failure::Network(e.to_string())
    }
}

#[cfg(test)]
pub(crate) mod testserver {
    //! A one-shot HTTP server on loopback for the backends' tests: it records the request
    //! it receives and answers with a canned status, headers and body.

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    pub struct Seen {
        pub request_line: String,
        pub headers: Vec<(String, String)>,
        pub body: String,
    }

    impl Seen {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }
    }

    /// Serve one request; returns the port and a receiver for what was seen.
    pub fn serve_once(
        status: u16,
        extra_headers: &[(&str, &str)],
        body: &str,
    ) -> (u16, mpsc::Receiver<Seen>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        let extra: Vec<(String, String)> = extra_headers
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let body = body.to_string();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut headers = Vec::new();
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end().to_string();
                if line.is_empty() {
                    break;
                }
                if let Some((n, v)) = line.split_once(':') {
                    if n.eq_ignore_ascii_case("content-length") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                    headers.push((n.trim().to_string(), v.trim().to_string()));
                }
            }
            let mut buf = vec![0u8; length];
            reader.read_exact(&mut buf).unwrap();
            let _ = tx.send(Seen {
                request_line: request_line.trim_end().to_string(),
                headers,
                body: String::from_utf8_lossy(&buf).into_owned(),
            });
            let mut out = stream;
            let mut head = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                body.len()
            );
            for (n, v) in &extra {
                head.push_str(&format!("{n}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = out.write_all(head.as_bytes());
            let _ = out.write_all(body.as_bytes());
        });
        (port, rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_machine_is_a_local_host() {
        for h in [
            "127.0.0.1",
            "127.1.2.3",
            "localhost",
            "LOCALHOST",
            "::1",
            "[::1]",
        ] {
            assert!(is_loopback(h), "{h}");
        }
        for h in ["192.168.1.10", "10.0.0.2", "example.com", "0.0.0.0", ""] {
            assert!(!is_loopback(h), "{h}");
        }
    }

    #[test]
    fn off_is_no_backend_and_a_foreign_local_host_is_refused() {
        let mut cfg = ExplainConfig::default();
        assert!(
            Backend::from_config(&cfg).unwrap().is_none(),
            "off by default"
        );
        cfg.backend = BackendKind::Local;
        cfg.local_model = "llama3.1".into();
        cfg.local_host = "192.168.1.5".into();
        assert!(Backend::from_config(&cfg).is_err());
        cfg.local_host = "127.0.0.1".into();
        cfg.local_model = " ".into();
        assert!(Backend::from_config(&cfg).is_err(), "a model is needed");
        cfg.local_model = "llama3.1".into();
        let b = Backend::from_config(&cfg).unwrap().unwrap();
        assert!(!b.is_remote());
        assert!(b.label().contains("127.0.0.1:11434") && b.label().contains("llama3.1"));
    }
}
