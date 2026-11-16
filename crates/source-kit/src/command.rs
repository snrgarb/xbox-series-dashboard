//! A command line as an Uninstall key's `UninstallString` holds one, taken apart into the program and its
//! arguments, so the shell can run it as `model::Start::Program`.
use std::path::PathBuf;

use model::Start;

/// The words of a command line as Windows takes them apart: split at spaces and tabs outside quotes, a quote
/// opening or closing a quoted part (and itself dropped), `\"` a quote kept. The program's own path, the first
/// word, keeps its backslashes as they are.
pub fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut any) = (false, false);
    let mut chars = line.trim().chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'"') && !words.is_empty() => {
                word.push('"');
                chars.next();
            }
            '"' => {
                quoted = !quoted;
                any = true;
            }
            ' ' | '\t' if !quoted => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
            }
            c => word.push(c),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    words
}

/// The command line as a program to run with its arguments, in its own folder; none for an empty line.
pub fn program(line: &str) -> Option<Start> {
    let mut words = words(line).into_iter();
    let path = words.next().filter(|path| !path.is_empty())?;
    Some(Start::Program { path: PathBuf::from(path), args: words.collect(), dir: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_line_is_its_program_and_its_arguments() {
        let line =
            r#""C:\Program Files\Common Files\EAInstaller\Cinderpeak Rally\Cleanup.exe" uninstall_game -autologging"#;
        assert_eq!(
            program(line),
            Some(Start::Program {
                path: PathBuf::from(r"C:\Program Files\Common Files\EAInstaller\Cinderpeak Rally\Cleanup.exe"),
                args: vec!["uninstall_game".into(), "-autologging".into()],
                dir: None,
            })
        );
        let battle = r#""C:\Program Files (x86)\Battle.net\Battle.net.exe" --uid=emberwatch --lang=enUS "--name=Ember \"W\"" """#;
        assert_eq!(
            words(battle),
            [
                r"C:\Program Files (x86)\Battle.net\Battle.net.exe",
                "--uid=emberwatch",
                "--lang=enUS",
                "--name=Ember \"W\"",
                ""
            ]
        );
        assert_eq!(words(r"C:\Tools\unins000.exe /SILENT"), [r"C:\Tools\unins000.exe", "/SILENT"]);
        assert_eq!(program("   "), None);
    }
}
