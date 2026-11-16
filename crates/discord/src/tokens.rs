//! Discord's OAuth2 token endpoint, `https://discord.com/api/oauth2/token`, asked on a thread of its own: the code
//! exchange after AUTHORIZE, and each refresh. A test points it at `standin discord`'s stand-in with
//! `--discord-token-url`; a test that names a pipe and no endpoint has none, and every request fails at once, so a
//! test never reaches the real one. Neither the secret nor a token is ever in a log line: an answer's body is not
//! logged at all, since a good one holds the tokens.
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::client::Tokens;
use crate::ipc::{Incoming, Wake};

/// Discord's own.
pub const DISCORD: &str = "https://discord.com/api/oauth2/token";

/// What a request with no endpoint answers.
pub const NONE_GIVEN: &str = "no token endpoint was given for this test";

pub struct HttpTokens {
    /// None: every request fails at once, with [`NONE_GIVEN`].
    url: Option<String>,
    incoming: Sender<Incoming>,
    wake: Wake,
}

impl HttpTokens {
    pub fn new(url: Option<String>, incoming: Sender<Incoming>, wake: Wake) -> Self {
        Self { url, incoming, wake }
    }
}

impl Tokens for HttpTokens {
    fn request(&mut self, client_id: &str, secret: &str, form: Vec<(&'static str, String)>) {
        let (incoming, wake) = (self.incoming.clone(), self.wake.clone());
        let Some(url) = self.url.clone() else {
            let _ = incoming.send(Incoming::Token(0, NONE_GIVEN.into()));
            wake();
            return;
        };
        let credentials = format!("Basic {}", base64(format!("{client_id}:{secret}").as_bytes()));
        let grant = form.iter().find(|(key, _)| *key == "grant_type").map(|(_, value)| value.clone());
        eprintln!("discord: asking the token endpoint ({})", grant.unwrap_or_default());
        let spawned = std::thread::Builder::new().name("discord tokens".into()).spawn(move || {
            let (status, body) = post(&url, &credentials, &form);
            // A body is never logged: a good one holds the tokens. What went wrong with no answer is in words of
            // our own.
            match status {
                0 => eprintln!("discord: the token endpoint did not answer: {body}"),
                200..=299 => eprintln!("discord: the token endpoint answered HTTP {status}"),
                _ => eprintln!("discord: the token endpoint refused, HTTP {status}"),
            }
            let _ = incoming.send(Incoming::Token(status, body));
            wake();
        });
        if spawned.is_err() {
            let _ = self.incoming.send(Incoming::Token(0, "no thread for the request".into()));
            (self.wake)();
        }
    }
}

/// The POST: the status and body, or 0 and what went wrong in words that name no address.
fn post(url: &str, credentials: &str, form: &[(&'static str, String)]) -> (u16, String) {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(concat!("xbox-series-dashboard/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let pairs = form.iter().map(|(key, value)| (*key, value.as_str()));
    let response = agent.post(url).header("Authorization", credentials).send_form(pairs);
    match response {
        Ok(mut response) => {
            let status = response.status().as_u16();
            let body = response.body_mut().with_config().limit(64 * 1024).read_to_string();
            (status, body.unwrap_or_default())
        }
        Err(e) => (0, unanswered(&e)),
    }
}

fn unanswered(error: &ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => "Discord took too long to answer".into(),
        ureq::Error::HostNotFound => "Discord could not be found: is the PC online?".into(),
        ureq::Error::ConnectionFailed | ureq::Error::Io(_) => "Discord could not be reached".into(),
        _ => "Discord's answer could not be read".into(),
    }
}

/// Standard base64, with padding: HTTP Basic's.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Standard base64 read back; None for anything else.
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let text = text.trim().trim_end_matches('=').as_bytes();
    let mut out = Vec::new();
    for chunk in text.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= value(c)? << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..chunk.len().saturating_sub(1)]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_the_standard_one() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"1290:s3cr3t"), "MTI5MDpzM2NyM3Q=");
        for text in ["", "f", "fo", "foo", "foob", "1290:s3cr3t"] {
            assert_eq!(unbase64(&base64(text.as_bytes())).unwrap(), text.as_bytes());
        }
    }

    #[test]
    fn with_no_endpoint_a_request_fails_at_once() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut tokens = HttpTokens::new(None, sender, std::sync::Arc::new(|| {}));
        tokens.request("1", "s", vec![("grant_type", "refresh_token".into())]);
        assert_eq!(receiver.try_recv(), Ok(Incoming::Token(0, NONE_GIVEN.into())));
    }
}
