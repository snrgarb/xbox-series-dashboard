//! As much of YAML as the product configurations in Ubisoft Connect's cache are written in: block mappings and
//! block lists by their indent, plain and quoted scalars, and `|` or `>` text, read into a tree. A flow list or
//! mapping (`[a, b]`, `{}`) is kept as the text it is, and anchors, tags and documents are not read: nothing the
//! source looks at is written that way in the files that were read (docs/research/stores.md, "Ubisoft Connect").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Scalar(String),
    List(Vec<Node>),
    Map(Vec<(String, Node)>),
}

impl Node {
    /// The value under `key`, when this is a mapping that has it.
    pub fn get(&self, key: &str) -> Option<&Node> {
        let Node::Map(entries) = self else { return None };
        entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value at a path of keys.
    pub fn at(&self, path: &[&str]) -> Option<&Node> {
        path.iter().try_fold(self, |node, key| node.get(key))
    }

    pub fn text(&self) -> Option<&str> {
        let Node::Scalar(text) = self else { return None };
        Some(text.as_str())
    }

    pub fn items(&self) -> &[Node] {
        match self {
            Node::List(items) => items,
            _ => &[],
        }
    }

    /// Whether this is `true` (or `yes`).
    pub fn truthy(&self) -> bool {
        self.text().is_some_and(|t| t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes"))
    }
}

#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    text: String,
}

/// The text as a tree: a mapping at the top, as these files are.
pub fn parse(text: &str) -> Node {
    let mut lines: Vec<Line> = text
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .filter(|line| !matches!(line.trim(), "---" | "..."))
        .map(|line| {
            let indent = line.len() - line.trim_start_matches(' ').len();
            Line { indent, text: line.trim().to_string() }
        })
        .collect();
    let mut at = 0;
    let indent = lines.first().map_or(0, |line| line.indent);
    block(&mut lines, &mut at, indent)
}

/// The block that starts at `lines[*at]` and goes on while lines are at `indent` (a mapping's keys or a list's
/// items) or deeper (their values).
fn block(lines: &mut [Line], at: &mut usize, indent: usize) -> Node {
    if *at >= lines.len() || lines[*at].indent < indent {
        return Node::Scalar(String::new());
    }
    if is_item(&lines[*at].text) {
        let mut items = Vec::new();
        while *at < lines.len() && lines[*at].indent == indent && is_item(&lines[*at].text) {
            let rest = lines[*at].text[1..].trim_start().to_string();
            if rest.is_empty() {
                *at += 1;
                items.push(nested(lines, at, indent, true));
            } else if split_key(&rest).is_some() {
                // "- key: value": a mapping whose first key is on the item's line, its others under it.
                let inner = indent + (lines[*at].text.len() - rest.len());
                lines[*at] = Line { indent: inner, text: rest };
                items.push(block(lines, at, inner));
            } else {
                *at += 1;
                items.push(Node::Scalar(scalar(&rest)));
            }
        }
        return Node::List(items);
    }
    let mut entries = Vec::new();
    while *at < lines.len() && lines[*at].indent == indent && !is_item(&lines[*at].text) {
        let text = lines[*at].text.clone();
        *at += 1;
        let Some((key, value)) = split_key(&text) else {
            while *at < lines.len() && lines[*at].indent > indent {
                *at += 1;
            }
            continue;
        };
        let value = match value.trim() {
            "" => nested(lines, at, indent, false),
            "|" | "|-" | "|+" | ">" | ">-" | ">+" => {
                let mut text = Vec::new();
                while *at < lines.len() && lines[*at].indent > indent {
                    text.push(lines[*at].text.clone());
                    *at += 1;
                }
                Node::Scalar(text.join(if value.trim().starts_with('|') { "\n" } else { " " }))
            }
            value => Node::Scalar(scalar(value)),
        };
        entries.push((key, value));
        // A line deeper than this block that is no key's value: passed over.
        while *at < lines.len() && lines[*at].indent > indent {
            *at += 1;
        }
    }
    Node::Map(entries)
}

/// What is under a key with nothing after its colon (or an item with nothing after its dash): the deeper block, or
/// a list at the key's own indent, as YAML allows; else nothing.
fn nested(lines: &mut [Line], at: &mut usize, indent: usize, item: bool) -> Node {
    match lines.get(*at) {
        Some(next) if next.indent > indent => {
            let deeper = next.indent;
            block(lines, at, deeper)
        }
        Some(next) if !item && next.indent == indent && is_item(&next.text) => block(lines, at, indent),
        _ => Node::Scalar(String::new()),
    }
}

fn is_item(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

/// `key: value` as the key and what follows the colon; none for a line that is not one.
fn split_key(text: &str) -> Option<(String, &str)> {
    let (key, value) = if let Some(quote @ ('"' | '\'')) = text.chars().next() {
        let end = text[1..].find(quote)? + 1;
        let rest = text[end + 1..].strip_prefix(':')?;
        (text[1..end].to_string(), rest)
    } else {
        let colon = text.find(": ").or_else(|| text.ends_with(':').then(|| text.len() - 1))?;
        (text[..colon].trim().to_string(), &text[colon + 1..])
    };
    if !value.is_empty() && !value.starts_with(' ') {
        return None;
    }
    Some((key, value))
}

/// A scalar's text: unquoted, a trailing comment off a plain one.
fn scalar(text: &str) -> String {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return inner.replace("\\\"", "\"").replace("\\\\", "\\");
    }
    if let Some(inner) = text.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    match text.find(" #") {
        Some(comment) => text[..comment].trim_end().to_string(),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_lists_and_text_read_into_a_tree() {
        let text = "version: 2.0
root:
  name: l1
  thumb_image: \"t1\"
  is_ulc: false # a comment
  addons:
    - id: 90212
      name: Soundtrack
    - id: 90213
  start_game:
    online:
      executables:
      - path:
          relative: Harborlight.exe
        working_directory:
          register: HKEY_LOCAL_MACHINE\\SOFTWARE\\Ubisoft\\Launcher\\Installs\\90210\\InstallDir
  description: |
    Two lines
    of it
  tags: [a, b]
  'quoted key': 'it''s'
localizations:
  default:
    l1: Harborlight
";
        let tree = parse(text);
        assert_eq!(tree.at(&["version"]).and_then(Node::text), Some("2.0"));
        let root = tree.get("root").unwrap();
        assert_eq!(root.at(&["name"]).and_then(Node::text), Some("l1"));
        assert_eq!(root.at(&["thumb_image"]).and_then(Node::text), Some("t1"));
        assert!(!root.get("is_ulc").unwrap().truthy());
        let addons: Vec<&str> =
            root.get("addons").unwrap().items().iter().filter_map(|a| a.get("id")?.text()).collect();
        assert_eq!(addons, ["90212", "90213"]);
        let executables = root.at(&["start_game", "online", "executables"]).unwrap().items();
        assert_eq!(executables.len(), 1);
        assert_eq!(executables[0].at(&["path", "relative"]).and_then(Node::text), Some("Harborlight.exe"));
        assert!(executables[0].at(&["working_directory", "register"]).is_some());
        assert_eq!(root.at(&["description"]).and_then(Node::text), Some("Two lines\nof it"));
        assert_eq!(root.at(&["tags"]).and_then(Node::text), Some("[a, b]"));
        assert_eq!(root.at(&["quoted key"]).and_then(Node::text), Some("it's"));
        assert_eq!(tree.at(&["localizations", "default", "l1"]).and_then(Node::text), Some("Harborlight"));
        assert_eq!(tree.at(&["root", "nothing"]), None);
    }

    #[test]
    fn what_it_cannot_read_it_passes_over() {
        let tree = parse("a: 1\n  stray line\nb: 2\nnot a key\nc: http://x\n");
        assert_eq!(tree.get("b").and_then(Node::text), Some("2"));
        assert_eq!(tree.get("c").and_then(Node::text), Some("http://x"));
        assert_eq!(parse(""), Node::Scalar(String::new()));
    }
}
