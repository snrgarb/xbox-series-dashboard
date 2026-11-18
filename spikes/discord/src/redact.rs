//! What of an address may be logged: its scheme, host and path. Never its query or fragment (where sign-in codes,
//! tokens and e-mail links carry their secrets), never a user or password in it, and no long number or long opaque
//! segment of the path (a conversation's id, a reset or verification token): those become `<id>` and `<long>`.
pub fn address(url: &str) -> String {
    let url = url.trim();
    let Some((scheme, rest)) = url.split_once("://") else {
        // about:blank, data:..., javascript:...: the scheme alone, and about:'s page name.
        return match url.split_once(':') {
            Some(("about", page)) => format!("about:{}", cut(page)),
            Some((scheme, _)) => format!("{scheme}:"),
            None => "(not an address)".into(),
        };
    };
    let rest = cut(rest);
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let host = authority.rsplit('@').next().unwrap_or("");
    let path: Vec<String> = path.split('/').map(segment).collect();
    format!("{scheme}://{host}{}", path.join("/"))
}

fn cut(s: &str) -> &str {
    let end = s.find(['?', '#']).unwrap_or(s.len());
    &s[..end]
}

fn segment(s: &str) -> String {
    if s.len() >= 6 && s.chars().all(|c| c.is_ascii_digit()) {
        "<id>".into()
    } else if s.len() > 32 {
        "<long>".into()
    } else {
        s.into()
    }
}

#[cfg(test)]
mod tests {
    use super::address;

    #[test]
    fn host_and_path_only() {
        assert_eq!(address("https://discord.com/login?redirect_to=%2Fchannels"), "https://discord.com/login");
        assert_eq!(address("https://discord.com/channels/@me#frag"), "https://discord.com/channels/@me");
        assert_eq!(address("https://user:pass@example.com/x?y"), "https://example.com/x");
        assert_eq!(address("https://discord.com"), "https://discord.com");
    }

    #[test]
    fn ids_and_tokens_in_paths_are_hidden() {
        assert_eq!(
            address("https://discord.com/channels/@me/123456789012345678"),
            "https://discord.com/channels/@me/<id>"
        );
        assert_eq!(
            address("https://discord.com/reset/abcdefghijklmnopqrstuvwxyz0123456789ABCD"),
            "https://discord.com/reset/<long>"
        );
    }

    #[test]
    fn other_schemes() {
        assert_eq!(address("about:blank"), "about:blank");
        assert_eq!(address("data:text/html,<b>secret</b>"), "data:");
        assert_eq!(address("nothing"), "(not an address)");
    }
}
