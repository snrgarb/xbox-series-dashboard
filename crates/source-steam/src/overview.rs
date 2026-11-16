//! Steam's own download overview, read over its CEF debugger where the user has it on (`crate::cef`): what its
//! Downloads page shows. `SteamClient.Downloads.RegisterForDownloadOverview` gives it at once and on each change:
//!
//! ```text
//! {"update_appid":60,"update_is_upload":false,"update_network_bytes_per_second":16465111,
//!  "update_peak_network_bytes_per_second":112096665,"update_disc_bytes_per_second":0,
//!  "update_state":"Downloading","update_state_flags":97,
//!  "progress":[{"bytes_in_progress":2018,"bytes_total":2018,"estimated_time_remaining_sec":0}, ...], ...}
//! ```
//!
//! Seen on the real machine installing Ricochet (2026-11-16, #132). What is known from that:
//!
//! - `update_appid` is the app Steam is working on, and stays the app asked for while Steam fetches the shared
//!   content it needs: Ricochet's manifest said installed (flags 4) ten seconds before Steam had done with
//!   Half-Life's 564 MB, all under `update_appid` 60, `update_state` "Updating", then "Installing". So an app is not
//!   installed while the overview names it ([`Overview::working_on`]).
//! - `update_network_bytes_per_second` is Steam's own download speed, its Downloads page's.
//!
//! What is not known: what each `progress` entry counts. The entries seen had `bytes_in_progress` equal to
//! `bytes_total` from the first look (2018, then Ricochet's 17605107 bytes to stage, then Half-Life's 564102621),
//! and the rest was cut off in the log. So bytes are taken from the overview only where it names them plainly
//! ([`Overview::bytes`]): `update_bytes_downloaded` of `update_bytes_to_download`, which older clients gave, else
//! the one `progress` entry whose whole is the manifest's `BytesToDownload`, read as done of the whole. Anything
//! else leaves the bytes to the live count (`crate::live`). `--log-queue-sources` prints the overview whole, so
//! the next real install says which entry is which.
use crate::cef;

/// The script that gives the client's download overview once, or `none` after two seconds.
pub const SCRIPT: &str = "new Promise(resolve => { let reg = null; \
    const done = text => { if (reg) reg.unregister(); resolve(text); }; \
    reg = SteamClient.Downloads.RegisterForDownloadOverview(o => done(JSON.stringify(o))); \
    setTimeout(() => done('none'), 2000); })";

/// One entry of the overview's `progress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub in_progress: u64,
    pub total: u64,
}

/// The overview, as far as it is read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Overview {
    /// The app Steam is updating or installing; 0 for none.
    pub appid: u32,
    /// "Downloading", "Updating", "Installing", ...: empty when none is given.
    pub state: String,
    /// Steam's own download speed, bytes a second.
    pub network_bytes_per_second: u64,
    /// `update_bytes_downloaded` and `update_bytes_to_download`, where the client gives them.
    pub downloaded: Option<(u64, u64)>,
    pub progress: Vec<Progress>,
    /// The overview as Steam gave it, for the log.
    pub raw: String,
}

impl Overview {
    /// The overview from the text Steam gave, None for anything else (`none`: no answer).
    pub fn parse(text: &str) -> Option<Overview> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        let number = |key: &str| object.get(key).and_then(|value| value.as_u64());
        let progress = object
            .get("progress")
            .and_then(|progress| progress.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| {
                        let field = |key: &str| entry.get(key).and_then(|value| value.as_u64());
                        Some(Progress { in_progress: field("bytes_in_progress")?, total: field("bytes_total")? })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let downloaded = match (number("update_bytes_downloaded"), number("update_bytes_to_download")) {
            (Some(done), Some(total)) if total > 0 => Some((done.min(total), total)),
            _ => None,
        };
        Some(Overview {
            appid: number("update_appid").and_then(|appid| u32::try_from(appid).ok()).unwrap_or(0),
            state: object.get("update_state").and_then(|state| state.as_str()).unwrap_or_default().to_string(),
            network_bytes_per_second: number("update_network_bytes_per_second").unwrap_or(0),
            downloaded,
            progress,
            raw: text.to_string(),
        })
    }

    /// Whether Steam is at work on `appid`: it names it, in a state that is not none.
    pub fn working_on(&self, appid: u32) -> bool {
        self.appid == appid && appid != 0 && !self.state.is_empty() && !self.state.eq_ignore_ascii_case("none")
    }

    /// The bytes downloaded of the whole for `appid`, where the overview names them plainly (the module's
    /// rules): `to_download` is the manifest's `BytesToDownload`. With which field they came from, for the log.
    pub fn bytes(&self, appid: u32, to_download: u64) -> Option<((u64, u64), &'static str)> {
        if !self.working_on(appid) {
            return None;
        }
        if let Some(bytes) = self.downloaded {
            return Some((bytes, "update_bytes_downloaded"));
        }
        let mut matching = self.progress.iter().filter(|entry| to_download > 0 && entry.total == to_download);
        match (matching.next(), matching.next()) {
            (Some(entry), None) => Some(((entry.in_progress.min(entry.total), entry.total), "progress")),
            _ => None,
        }
    }

    /// Steam's speed, in bits a second, while it is at work on `appid`.
    pub fn speed(&self, appid: u32) -> Option<u64> {
        self.working_on(appid).then_some(self.network_bytes_per_second.saturating_mul(8))
    }
}

/// The overview from the Steam whose debugger answers on this PC, or why there is none.
pub fn read() -> Result<Overview, String> {
    match cef::evaluate(&cef::PORTS, cef::SHARED, SCRIPT) {
        Ok(cef::Evaluated::Value(text)) => Overview::parse(&text).ok_or_else(|| format!("no overview: {text}")),
        Ok(other) => Err(format!("{other:?}")),
        Err(why) => Err(format!("unreachable ({why})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As the real machine gave it as Ricochet downloaded, its tail cut off by the log there.
    const SEEN: &str = r#"{"remote_client_id":"0","update_appid":60,"update_is_upload":false,
        "update_network_bytes_per_second":16465111,"update_peak_network_bytes_per_second":112096665,
        "update_disc_bytes_per_second":0,"update_state":"Downloading","update_state_flags":97,
        "progress":[{"bytes_in_progress":2018,"bytes_total":2018,"estimated_time_remaining_sec":0},
        {"bytes_in_progress":17605107,"bytes_total":17605107,"estimated_time_remaining_sec":3}]}"#;

    #[test]
    fn steams_overview_names_the_app_it_is_at_and_its_speed() {
        let overview = Overview::parse(SEEN).unwrap();
        assert_eq!((overview.appid, overview.state.as_str()), (60, "Downloading"));
        assert!(overview.working_on(60) && !overview.working_on(70));
        assert_eq!(overview.speed(60), Some(131_720_888));
        assert_eq!(overview.speed(70), None);
        assert_eq!(overview.progress.len(), 2);
        // No entry is the manifest's 5540640 to download: the bytes are not the overview's to say.
        assert_eq!(overview.bytes(60, 5_540_640), None);
        assert_eq!(Overview::parse("none"), None);
        let idle = Overview::parse(r#"{"update_appid":0,"update_state":"None"}"#).unwrap();
        assert!(!idle.working_on(0));
    }

    #[test]
    fn bytes_come_only_where_the_overview_names_them_plainly() {
        let older = r#"{"update_appid":620,"update_state":"Downloading","update_bytes_downloaded":500,
            "update_bytes_to_download":1000,"progress":[]}"#;
        assert_eq!(Overview::parse(older).unwrap().bytes(620, 7), Some(((500, 1000), "update_bytes_downloaded")));
        let entry = r#"{"update_appid":620,"update_state":"Downloading","progress":[
            {"bytes_in_progress":10,"bytes_total":10},{"bytes_in_progress":300,"bytes_total":1000}]}"#;
        let overview = Overview::parse(entry).unwrap();
        assert_eq!(overview.bytes(620, 1000), Some(((300, 1000), "progress")));
        assert_eq!(overview.bytes(620, 0), None);
        assert_eq!(overview.bytes(70, 1000), None);
        // Two entries of the same whole: which is which is not known.
        let twice = r#"{"update_appid":620,"update_state":"Downloading","progress":[
            {"bytes_in_progress":10,"bytes_total":1000},{"bytes_in_progress":300,"bytes_total":1000}]}"#;
        assert_eq!(Overview::parse(twice).unwrap().bytes(620, 1000), None);
    }
}
