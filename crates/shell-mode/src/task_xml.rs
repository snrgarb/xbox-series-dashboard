//! A task's XML, as schtasks reads and writes it: what shell mode registers, and what it reads back of one.
use crate::system::Task;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The text of the first `<tag>` element, if there is one.
fn element(xml: &str, tag: &str) -> Option<String> {
    let start = xml.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = xml[start..].find(&format!("</{tag}>"))? + start;
    Some(unescape(&xml[start..end]))
}

/// `PT90S`, `PT1M30S`: seconds.
fn seconds(duration: &str) -> u32 {
    let mut total = 0;
    let mut number = 0;
    for c in duration.trim_start_matches("PT").chars() {
        match c {
            '0'..='9' => number = number * 10 + c.to_digit(10).unwrap_or(0),
            'H' => (total, number) = (total + number * 3600, 0),
            'M' => (total, number) = (total + number * 60, 0),
            'S' => (total, number) = (total + number, 0),
            _ => number = 0,
        }
    }
    total
}

pub fn read(xml: &str) -> Task {
    Task {
        command: element(xml, "Command").unwrap_or_default(),
        arguments: element(xml, "Arguments").unwrap_or_default(),
        delay_seconds: element(xml, "Delay").map_or(0, |delay| seconds(&delay)),
    }
}

/// The task as the Bundle registers its safety net: at this user's sign-in after the delay, as the user, not
/// elevated, five minutes at most.
pub fn write(task: &Task, user: &str) -> String {
    let user = escape(user);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>The Xbox Series Dashboard's safety net: starts explorer if none runs {delay} s after sign-in.</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId><Delay>PT{delay}S</Delay></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><ExecutionTimeLimit>PT5M</ExecutionTimeLimit><Enabled>true</Enabled></Settings>
  <Actions Context="Author"><Exec><Command>{command}</Command><Arguments>{arguments}</Arguments></Exec></Actions>
</Task>
"#,
        delay = task.delay_seconds,
        command = escape(&task.command),
        arguments = escape(&task.arguments),
    )
}

/// Whether schtasks' CSV list of every task holds `name` in the root folder.
pub fn lists(csv: &str, name: &str) -> bool {
    let quoted = format!("\"\\{}\",", name.to_ascii_lowercase());
    csv.lines().any(|line| line.trim_start().to_ascii_lowercase().starts_with(&quoted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_written_reads_back_as_it_was() {
        let task = Task {
            command: r"C:\Windows\System32\conhost.exe".into(),
            arguments: r#"--headless powershell.exe -Command "if (1 -lt 2) { 'a & b' }""#.into(),
            delay_seconds: 90,
        };
        let xml = write(&task, r"PC\A User");
        assert!(xml.contains("<Delay>PT90S</Delay>") && xml.contains(r"<UserId>PC\A User</UserId>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert_eq!(read(&xml), task);
        // Windows writes the delay back as it likes.
        assert_eq!(read(&xml.replace("PT90S", "PT1M30S")).delay_seconds, 90);
    }

    #[test]
    fn the_list_of_tasks_names_one_in_the_root_folder_only() {
        let csv = "\"\\XboxSeriesDashboard.SafetyNet\",\"N/A\",\"Ready\"\n\"\\Microsoft\\Other\",\"N/A\",\"Ready\"";
        assert!(lists(csv, "XboxSeriesDashboard.SafetyNet"));
        assert!(!lists(csv, "Other"));
    }

    #[test]
    fn the_task_runs_as_the_users_sid_in_its_trigger_and_its_principal() {
        let task = Task { command: "conhost.exe".into(), arguments: String::new(), delay_seconds: 90 };
        let xml = write(&task, "S-1-5-21-1004336348-1177238915-682003330-1001");
        assert_eq!(xml.matches("<UserId>S-1-5-21-1004336348-1177238915-682003330-1001</UserId>").count(), 2);
        assert!(!xml.contains("WORKGROUP"));
    }
}
