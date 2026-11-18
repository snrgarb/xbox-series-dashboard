//! Requests to the Agent on 127.0.0.1, each said in one line of the log with what came back, the session's token
//! never among them: it is written `<token>` wherever it would appear, in a request or an answer.
use std::time::Duration;

use serde_json::Value;

/// What the Agent answered: its status and its body, as JSON where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub status: u16,
    pub body: Value,
}

impl Answer {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// A text field of the answer.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.body.get(key).and_then(Value::as_str)
    }
}

/// One HTTP session with the Agent on `port`: the user agent it expects, and its token once it has given one.
pub struct Http {
    agent: ureq::Agent,
    port: u16,
    pub token: Option<String>,
}

/// The user agent the Agent answers (barncastle's `AgentApp.cs`, HSCentric's `BattleNetAgentClient.cs`).
pub const USER_AGENT: &str = "phoenix-agent/1.0";

impl Http {
    pub fn new(port: u16) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(USER_AGENT)
            .build()
            .into();
        Self { agent, port, token: None }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `method` on `path`, with `body` as JSON where there is one. Said in the log as `battlenet: agent: <what>:
    /// <METHOD> <path> <body> -> <status> <answer>`; an answer that is not JSON is kept as its text.
    pub fn call(&self, what: &str, method: &str, path: &str, body: Option<&Value>) -> Result<Answer, String> {
        self.call_said(what, method, path, body, true)
    }

    /// As [`call`](Http::call), said in the log only where `say` is, or where it went wrong (no answer, or a
    /// status that is not 2xx): for the progress asked once a second, whose lines the follower says itself.
    pub fn call_said(
        &self,
        what: &str,
        method: &str,
        path: &str,
        body: Option<&Value>,
        say: bool,
    ) -> Result<Answer, String> {
        let url = format!("http://127.0.0.1:{}{path}", self.port);
        let sent = body.map(Value::to_string);
        let result = self.send(method, &url, sent.as_deref());
        let asked = match &sent {
            Some(sent) => format!("{method} {path} {}", self.mask(sent)),
            None => format!("{method} {path}"),
        };
        match result {
            Ok(answer) if !say && answer.ok() => Ok(answer),
            Ok(answer) => {
                crate::log(&format!(
                    "agent: {what}: {asked} -> {} {}",
                    answer.status,
                    self.mask(&answer.body.to_string())
                ));
                Ok(answer)
            }
            Err(why) => {
                crate::log(&format!("agent: {what}: {asked} -> no answer: {why}"));
                Err(why)
            }
        }
    }

    fn send(&self, method: &str, url: &str, body: Option<&str>) -> Result<Answer, String> {
        let token = self.token.as_deref();
        let response = match (method, body) {
            ("GET", _) => with_token(self.agent.get(url), token).call(),
            ("DELETE", _) => with_token(self.agent.delete(url), token).call(),
            ("POST", body) => {
                with_token(self.agent.post(url), token).content_type("application/json").send(body.unwrap_or("{}"))
            }
            ("PUT", body) => {
                with_token(self.agent.put(url), token).content_type("application/json").send(body.unwrap_or("{}"))
            }
            (other, _) => return Err(format!("{other} is not a method asked of the Agent")),
        };
        let mut response = response.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let text = response.body_mut().with_config().limit(4 << 20).read_to_string().unwrap_or_default();
        let body = serde_json::from_str(&text).unwrap_or(if text.trim().is_empty() {
            Value::Null
        } else {
            Value::String(text)
        });
        Ok(Answer { status, body })
    }

    /// `text` with the token written `<token>`.
    pub fn mask(&self, text: &str) -> String {
        mask(text, self.token.as_deref())
    }
}

fn with_token<B>(request: ureq::RequestBuilder<B>, token: Option<&str>) -> ureq::RequestBuilder<B> {
    match token {
        Some(token) => request.header("Authorization", token),
        None => request,
    }
}

/// `text` with `token`, and the value of every `authorization` field, written `<token>`.
pub fn mask(text: &str, token: Option<&str>) -> String {
    let mut text = match token.filter(|t| !t.is_empty()) {
        Some(token) => text.replace(token, "<token>"),
        None => text.to_string(),
    };
    // A token the Agent gives in an answer, before it is the session's: `"authorization":"..."`.
    let mut from = 0;
    while let Some(at) = text[from..].to_ascii_lowercase().find("\"authorization\":\"").map(|i| i + from) {
        let start = at + "\"authorization\":\"".len();
        let Some(end) = text[start..].find('"').map(|i| i + start) else { break };
        text.replace_range(start..end, "<token>");
        from = start + "<token>".len();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_never_said() {
        let answer = r#"{"authorization":"A1B2-secret","pid":12,"state":1001}"#;
        assert_eq!(mask(answer, None), r#"{"authorization":"<token>","pid":12,"state":1001}"#);
        assert_eq!(
            mask("Authorization: A1B2-secret, again A1B2-secret", Some("A1B2-secret")),
            "Authorization: <token>, again <token>"
        );
        assert_eq!(
            mask(r#"{"Authorization":"x","b":{"authorization":"y"}}"#, None),
            r#"{"Authorization":"<token>","b":{"authorization":"<token>"}}"#
        );
    }
}
