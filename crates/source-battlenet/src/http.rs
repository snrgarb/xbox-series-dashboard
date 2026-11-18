//! Requests to the Agent on 127.0.0.1, each said in one line of the log with what came back, the session's token
//! never among them: it is written `<token>` wherever it would appear, in a request or an answer. Nor the account's
//! and Windows' names for the user, which `GET /agent` gives beside the token (`session`, `user_id`, a Windows SID,
//! and `user_name`, as the TV PC's Agent answered): each written `<masked>` (`SECRET`).
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
    /// One more header sent with every request: the `pid` asked of an Agent the Dashboard did not start
    /// (`agent::calls::PID_HEADER`).
    pub extra: Option<(&'static str, String)>,
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
        Self { agent, port, token: None, extra: None }
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
    /// status that is not 2xx).
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
        let shown = body.map(|body| masked(body).to_string());
        let result = self.send(method, &url, sent.as_deref());
        let asked = match &shown {
            Some(sent) => format!("{method} {path} {}", self.mask(sent)),
            None => format!("{method} {path}"),
        };
        match result {
            Ok(answer) if !say && answer.ok() => Ok(answer),
            Ok(answer) => {
                crate::log(&format!(
                    "agent: {what}: {asked} -> {} {}",
                    answer.status,
                    self.mask(&masked(&answer.body).to_string())
                ));
                Ok(answer)
            }
            Err(why) => {
                crate::log(&format!("agent: {what}: {asked} -> no answer: {why}"));
                Err(why)
            }
        }
    }

    /// `method` on `path`, said nowhere: for the progress asked once a second, whose answers the follower says
    /// itself (`installing`).
    pub fn fetch(&self, method: &str, path: &str) -> Result<Answer, String> {
        self.send(method, &format!("http://127.0.0.1:{}{path}", self.port), None)
    }

    fn send(&self, method: &str, url: &str, body: Option<&str>) -> Result<Answer, String> {
        let token = self.token.as_deref();
        let extra = self.extra.as_ref().map(|(name, value)| (*name, value.as_str()));
        let response = match (method, body) {
            ("GET", _) => with_token(self.agent.get(url), token, extra).call(),
            ("DELETE", _) => with_token(self.agent.delete(url), token, extra).call(),
            ("POST", body) => with_token(self.agent.post(url), token, extra)
                .content_type("application/json")
                .send(body.unwrap_or("{}")),
            ("PUT", body) => with_token(self.agent.put(url), token, extra)
                .content_type("application/json")
                .send(body.unwrap_or("{}")),
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

fn with_token<B>(
    request: ureq::RequestBuilder<B>,
    token: Option<&str>,
    extra: Option<(&str, &str)>,
) -> ureq::RequestBuilder<B> {
    let request = match extra {
        Some((name, value)) => request.header(name, value),
        None => request,
    };
    match token {
        Some(token) => request.header("Authorization", token),
        None => request,
    }
}

/// The fields of the Agent's answers that name the user or open a session, never said: `GET /agent`'s `session`,
/// `user_id` (a Windows SID) and `user_name` (the TV PC's Agent, 2.42.1.9886). The token is `<token>` (`mask`).
pub const SECRET: &[&str] = &["session", "user_id", "user_name"];

/// `value` with every field [`SECRET`] names written `<masked>`, and every `authorization` `<token>`, at any depth.
pub fn masked(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, field)| {
                    let key_lower = key.to_ascii_lowercase();
                    let shown = if key_lower == "authorization" {
                        Value::String("<token>".into())
                    } else if SECRET.contains(&key_lower.as_str()) && !field.is_null() {
                        Value::String("<masked>".into())
                    } else {
                        masked(field)
                    };
                    (key.clone(), shown)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(masked).collect()),
        other => other.clone(),
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
    fn the_users_names_and_session_are_never_said() {
        let answer = serde_json::json!({"authorization": "T0K3N", "opt_in_feedback": true, "pid": 9, "region": "us",
            "session": "1234567890", "state": 1007, "type": "retail", "user_id": "S-1-5-21-1-2-3-1001",
            "user_name": "someone", "inner": [{"Session": 5}]});
        let said = masked(&answer).to_string();
        for secret in ["T0K3N", "1234567890", "S-1-5-21", "someone", "\"Session\":5"] {
            assert!(!said.contains(secret), "{said}");
        }
        assert!(said.contains("\"user_id\":\"<masked>\"") && said.contains("\"authorization\":\"<token>\""));
        assert!(said.contains("\"state\":1007") && said.contains("\"region\":\"us\""));
    }

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
