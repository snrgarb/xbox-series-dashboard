//! A licence agreement's text, for the Dashboard's own prompt (#230): read from the url Steam's `LoadEula` gives
//! (on the real client one on store.steampowered.com, `?l=english&cc=..&client=1`; on the stand-in its own page on
//! 127.0.0.1), on the install's own thread and never the window's, within [`TIME_LIMIT`], and the page's HTML
//! reduced to text that reads plainly ([`plain_text`]). What the store's real page looks like inside has not been
//! seen: the reduction is a general one, which keeps the words of the page's body and its paragraphs and lists, and
//! drops its scripts, styles, forms and the parts a page's frame is made of (`nav`, `header`, `footer`).
use std::time::Duration;

/// How long reading an agreement may take, all told.
pub const TIME_LIMIT: Duration = Duration::from_secs(15);
/// The most of a page read: an agreement is a few tens of kilobytes.
const MOST_BYTES: u64 = 4 << 20;

/// The agreement at `url` as plain text, or why it could not be had, in words that name no address.
pub fn fetch(url: &str) -> Result<String, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Steam gave no web address for it".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(TIME_LIMIT))
        .user_agent(concat!("xbox-series-dashboard/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|error| match error {
        ureq::Error::Timeout(_) => "Steam's store took too long to answer".to_string(),
        ureq::Error::HostNotFound => "Steam's store could not be found: is the PC online?".to_string(),
        ureq::Error::ConnectionFailed | ureq::Error::Io(_) => "Steam's store could not be reached".to_string(),
        _ => "Steam's store's answer could not be read".to_string(),
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("Steam's store answered {status}"));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(MOST_BYTES)
        .read_to_string()
        .map_err(|_| "Steam's store's answer could not be read".to_string())?;
    let text = plain_text(&body);
    if text.trim().is_empty() {
        return Err("the page Steam gave has no text".into());
    }
    Ok(text)
}

/// Elements whose content is never text to read.
const DROPPED: [&str; 12] =
    ["script", "style", "head", "noscript", "template", "svg", "nav", "header", "footer", "form", "button", "select"];
/// Elements that stand on lines of their own.
const BLOCKS: [&str; 22] = [
    "p",
    "div",
    "section",
    "article",
    "main",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "blockquote",
    "pre",
    "hr",
    "dl",
    "dt",
    "dd",
];

/// A page's HTML as the text a reader sees: its words with the spaces between them made single, a paragraph,
/// heading or list item a paragraph of its own, `<br>` a new line, a list item started with a bullet, entities read,
/// and nothing of what is dropped ([`DROPPED`]). Paragraphs are parted by one empty line.
pub fn plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut skip: Option<String> = None;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            if skip.is_none() {
                words(&mut out, rest);
            }
            break;
        };
        if skip.is_none() {
            words(&mut out, &rest[..open]);
        }
        rest = &rest[open..];
        // A comment, or a declaration such as the doctype.
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |end| &after[end + 3..]);
            continue;
        }
        let Some(close) = rest.find('>') else { break };
        let tag = &rest[1..close];
        rest = &rest[close + 1..];
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if let Some(skipped) = &skip {
            if closing && &name == skipped {
                skip = None;
            }
            continue;
        }
        if DROPPED.contains(&name.as_str()) {
            if !closing && !tag.ends_with('/') {
                skip = Some(name);
            }
            continue;
        }
        if name == "br" {
            out.push('\n');
        } else if BLOCKS.contains(&name.as_str()) {
            out.push_str("\n\n");
            if name == "li" && !closing {
                out.push_str("\u{2022} ");
            }
        } else if (name == "td" || name == "th") && closing {
            out.push(' ');
        }
    }
    tidy(&out)
}

/// Text between tags, its entities read, its runs of white space made one space.
fn words(out: &mut String, text: &str) {
    let text = entities(text);
    let mut space = out.ends_with([' ', '\n']) || out.is_empty();
    for c in text.chars() {
        if c.is_whitespace() && c != '\u{a0}' {
            if !space {
                out.push(' ');
                space = true;
            }
        } else {
            out.push(if c == '\u{a0}' { ' ' } else { c });
            space = false;
        }
    }
}

/// `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;`, a few typographic ones, and `&#N;` / `&#xH;`; any other
/// left as it is.
fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.find(';').filter(|&end| end <= 10);
        let read = end.and_then(|end| {
            let name = &rest[1..end];
            let c = match name {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => '\u{a0}',
                "copy" => '\u{a9}',
                "reg" => '\u{ae}',
                "trade" => '\u{2122}',
                "mdash" => '\u{2014}',
                "ndash" => '\u{2013}',
                "hellip" => '\u{2026}',
                "lsquo" => '\u{2018}',
                "rsquo" => '\u{2019}',
                "ldquo" => '\u{201c}',
                "rdquo" => '\u{201d}',
                "bull" => '\u{2022}',
                _ => {
                    let number = name.strip_prefix('#')?;
                    let code = match number.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => number.parse().ok()?,
                    };
                    char::from_u32(code)?
                }
            };
            Some((c, end))
        });
        match read {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Each line trimmed, and no more than one empty line between paragraphs, none at either end.
fn tidy(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() && out.last().is_none_or(|last| last.is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|last| last.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pages_html_reads_as_its_words_in_paragraphs() {
        let html = r#"<!DOCTYPE html><html><head><title>EULA</title><style>p { color: red }</style>
            <script>var a = "<p>not this</p>";</script></head>
            <body><header><a href="/">Store</a> | Community</header><nav>Menu</nav>
            <h1>End User   Licence
            Agreement</h1><!-- a comment <p>nor this</p> -->
            <p>By playing you agree &amp; accept&nbsp;these&#160;terms &#x2014; all &copy; Valve&reg;.</p>
            <ul><li>One</li><li>Two<br>lines</li></ul>
            <table><tr><td>A</td><td>B</td></tr></table>
            <p>Fish &chips; &unknown stays</p>
            <form><button>Accept</button></form><footer>Privacy policy</footer></body></html>"#;
        assert_eq!(
            plain_text(html),
            "End User Licence Agreement\n\nBy playing you agree & accept these terms \u{2014} all \u{a9} Valve\u{ae}.\n\n\
             \u{2022} One\n\n\u{2022} Two\nlines\n\nA B\n\nFish &chips; &unknown stays"
        );
    }

    #[test]
    fn plain_text_is_left_as_it_is_and_nothing_gives_nothing() {
        assert_eq!(plain_text("Just words,\n  wrapped."), "Just words, wrapped.");
        assert_eq!(plain_text(""), "");
        assert_eq!(plain_text("<html><head><title>x</title></head><body></body></html>"), "");
        // A tag left open at the end is not text.
        assert_eq!(plain_text("words <p"), "words");
    }

    #[test]
    fn an_address_that_is_not_a_web_address_is_never_asked() {
        assert!(fetch("file:///C:/eula.html").is_err());
        assert!(fetch("").is_err());
    }
}
