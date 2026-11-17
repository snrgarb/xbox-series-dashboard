//! What the Steam source keeps between its looks at the installs under way (`Steam::installs`), and what it makes
//! of each look: the one thing a source keeps, as the Bundle's `SteamDownloads` keeps its baselines.
//!
//! **How far, and how fast.** Where Steam's download overview names the app (`crate::overview`, with the user's
//! CEF debugger on), its speed is Steam's own, and its bytes too where the overview names them plainly. Else the
//! bytes are the live count of Steam's own writes (`crate::live`), and the speed is the Queue's to work out.
//!
//! **Not done until Steam is.** An app's manifest can say installed while Steam is still fetching the shared content
//! the app needs (Ricochet's said so ten seconds before Steam had done with Half-Life's 564 MB, #132). So an app
//! that was under way and whose manifest now says installed is still reported as installing, with no whole known,
//! while any of these holds:
//!
//! - Steam's overview names it (known: it is Steam's own word).
//! - The registry's `Apps\<appid>\Updating` says so, for the install the registry names (Steam's own word too,
//!   though on the real machine it was not seen set for the shared content).
//! - Its Workshop record says Steam still has items to fetch for it (`NeedsDownload`, #188): the game is playable
//!   then, which `Steam::installs` says (`InstallState::Extras`).
//! - Without the overview: another app turned up under way within [`SHARED_WITHIN`] seconds of this one and is still
//!   under way. That is taken as content this one needs, which Steam fetches as soon as it has the app's own:
//!   reasoned from one install, and wrong for a game the user happened to queue in Steam in those seconds, whose
//!   download then holds this one as installing until it is done. Nothing local says which app's update started
//!   another's.
//!
//! Each hold is said in the log as it starts and as it ends (`steam: <appid> ...`).
use std::collections::{HashMap, HashSet};

use model::{Install, InstallState};

use crate::installs::Counts;
use crate::live::{self, Baseline, Writes};
use crate::overview::Overview;

/// How soon after an app another must turn up under way to be taken as content it needs, in seconds.
pub const SHARED_WITHIN: f64 = 30.0;

#[derive(Debug, Default)]
pub struct Follow {
    baselines: HashMap<u32, Baseline>,
    /// When each app was first seen under way, on the caller's clock, while it is or is held.
    first_seen: HashMap<u32, f64>,
    /// The apps reported at the last look.
    reported: HashSet<u32>,
    /// The apps held as installing, with why.
    held: HashMap<u32, String>,
    /// Where each download's bytes came from at the last look, for a log line when that changes.
    counted_by: HashMap<u32, &'static str>,
}

impl Follow {
    /// Whether anything was reported at the last look: Steam's client is worth asking about even with no manifest
    /// under way, for an app held as installing.
    pub fn busy(&self) -> bool {
        !self.reported.is_empty()
    }

    /// Whether the app's manifest says installed and it was held as installing at the last look.
    pub fn held(&self, appid: u32) -> bool {
        self.held.contains_key(&appid)
    }

    /// What a look at the installs under way (`found`, from the manifests) comes to, at `at` seconds on the caller's
    /// clock; with Steam's overview where there is one, the client's writes where they can be had, and the
    /// registry's `Updating` (false where it is not to be asked). Says in the log what changed of the holds and of
    /// where each download's bytes come from.
    pub fn look(
        &mut self,
        found: Vec<(u32, InstallState, Counts)>,
        overview: Option<&Overview>,
        writes: Option<Writes>,
        updating: &dyn Fn(u32) -> bool,
        at: f64,
    ) -> Vec<Install> {
        self.look_with_workshop(found, overview, writes, updating, &|_| false, at)
    }

    /// [`look`](Follow::look), with Steam's Workshop record as one more word that an app is not done: `workshop`
    /// says whether the app's record has items still to fetch (#188), which holds it with no overview at all.
    pub fn look_with_workshop(
        &mut self,
        found: Vec<(u32, InstallState, Counts)>,
        overview: Option<&Overview>,
        writes: Option<Writes>,
        updating: &dyn Fn(u32) -> bool,
        workshop: &dyn Fn(u32) -> bool,
        at: f64,
    ) -> Vec<Install> {
        for (appid, ..) in &found {
            self.first_seen.entry(*appid).or_insert(at);
        }
        let under_way: HashSet<u32> = found.iter().map(|(appid, ..)| *appid).collect();

        // Those that were reported and whose manifest no longer says they are under way: held, or done.
        let mut held = HashMap::new();
        for &appid in self.reported.iter().chain(self.held.keys()).collect::<HashSet<_>>() {
            if under_way.contains(&appid) {
                continue;
            }
            let since = self.first_seen.get(&appid).copied().unwrap_or(at);
            let shared = under_way.iter().find(|other| {
                self.first_seen.get(other).is_some_and(|&seen| seen >= since && seen - since <= SHARED_WITHIN)
            });
            let why = if overview.is_some_and(|overview| overview.working_on(appid)) {
                Some(format!("Steam's download overview still names it, {}", overview.map_or("", |o| &o.state)))
            } else if updating(appid) {
                Some("the registry says it is still updating".to_string())
            } else if workshop(appid) {
                Some("its Workshop record says Steam still has items to fetch".to_string())
            } else if let (None, Some(other)) = (overview, shared) {
                Some(format!("{other} turned up under way just after it, taken as content it needs"))
            } else {
                None
            };
            if let Some(why) = why {
                if !self.held.contains_key(&appid) {
                    eprintln!("steam: {appid} reads installed, but it stays installing: {why}");
                }
                held.insert(appid, why);
            } else if self.held.contains_key(&appid) {
                eprintln!("steam: {appid} is done: nothing says Steam is still at it");
            }
        }
        self.held = held;

        self.baselines.retain(|appid, _| found.iter().any(|(id, state, _)| id == appid && state.downloading()));
        let mut installs: Vec<Install> = found
            .into_iter()
            .map(|(appid, state, counts)| {
                let (state, speed) = match state {
                    InstallState::Downloading { .. } => self.downloading(appid, &counts, overview, writes),
                    state => (state, None),
                };
                Install { source_id: appid.to_string(), state, speed }
            })
            .collect();
        let mut held: Vec<u32> = self.held.keys().copied().collect();
        held.sort_unstable();
        for appid in held {
            let speed = overview.and_then(|overview| overview.speed(appid));
            installs.push(Install {
                source_id: appid.to_string(),
                state: InstallState::Downloading { done: 0, total: 0 },
                speed,
            });
        }

        self.reported = installs.iter().filter_map(|install| install.source_id.parse().ok()).collect();
        let keep: HashSet<u32> = self.reported.clone();
        self.first_seen.retain(|appid, _| keep.contains(appid) || under_way.contains(appid));
        self.counted_by.retain(|appid, _| keep.contains(appid));
        installs
    }

    /// A running download's bytes and speed: the overview's where it names them, else the live count.
    fn downloading(
        &mut self,
        appid: u32,
        counts: &Counts,
        overview: Option<&Overview>,
        writes: Option<Writes>,
    ) -> (InstallState, Option<u64>) {
        let speed = overview.and_then(|overview| overview.speed(appid));
        let (state, by) = match overview.and_then(|overview| overview.bytes(appid, counts.to_download)) {
            Some(((done, total), field)) => {
                self.baselines.remove(&appid);
                // Never below what the manifest says is down.
                (InstallState::Downloading { done: done.max(counts.downloaded.min(total)), total }, field)
            }
            None => {
                let mut baseline = self.baselines.get(&appid).copied();
                let state = live::downloading(&mut baseline, counts, writes);
                match baseline {
                    Some(baseline) => self.baselines.insert(appid, baseline),
                    None => self.baselines.remove(&appid),
                };
                (state, if writes.is_some() { "steam.exe's writes" } else { "the manifest" })
            }
        };
        if self.counted_by.insert(appid, by) != Some(by) {
            let speed_by = if speed.is_some() { "Steam's overview" } else { "the Queue, from the bytes" };
            eprintln!("steam: {appid}'s bytes from {by}, its speed from {speed_by}");
        }
        (state, speed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(downloaded: u64, to_download: u64) -> Counts {
        Counts { flags: 1026, downloaded, to_download, staged: 0, to_stage: to_download * 3 }
    }

    fn overview(appid: u32, state: &str, progress: &str) -> Overview {
        let text = format!(
            r#"{{"update_appid":{appid},"update_state":"{state}","update_network_bytes_per_second":1000,"progress":[{progress}]}}"#
        );
        Overview::parse(&text).unwrap()
    }

    fn states(installs: &[Install]) -> Vec<(&str, InstallState, Option<u64>)> {
        installs.iter().map(|install| (install.source_id.as_str(), install.state, install.speed)).collect()
    }

    const NOT_UPDATING: &dyn Fn(u32) -> bool = &|_| false;
    const UNKNOWN: InstallState = InstallState::Downloading { done: 0, total: 0 };

    #[test]
    fn steams_overview_gives_the_speed_and_the_bytes_where_it_names_them() {
        let mut follow = Follow::default();
        let named = overview(60, "Downloading", r#"{"bytes_in_progress":2000,"bytes_total":5000}"#);
        let found = vec![(60, InstallState::Downloading { done: 0, total: 5000 }, counts(0, 5000))];
        let got = follow.look(found, Some(&named), None, NOT_UPDATING, 0.0);
        assert_eq!(states(&got), [("60", InstallState::Downloading { done: 2000, total: 5000 }, Some(8000))]);
        // Its bytes not named: the live count, with Steam's speed all the same.
        let unnamed = overview(60, "Downloading", r#"{"bytes_in_progress":9,"bytes_total":9}"#);
        let found = vec![(60, InstallState::Downloading { done: 100, total: 5000 }, counts(100, 5000))];
        let got = follow.look(found, Some(&unnamed), None, NOT_UPDATING, 1.0);
        assert_eq!(states(&got), [("60", InstallState::Downloading { done: 100, total: 5000 }, Some(8000))]);
    }

    #[test]
    fn an_app_whose_manifest_says_installed_stays_installing_while_steams_overview_names_it() {
        let mut follow = Follow::default();
        let found = vec![(60, InstallState::Downloading { done: 0, total: 5000 }, counts(0, 5000))];
        follow.look(found, None, None, NOT_UPDATING, 0.0);
        // Ricochet's manifest says installed; Steam goes on, under 60, with Half-Life's content (70).
        let updating = overview(60, "Updating", r#"{"bytes_in_progress":564102621,"bytes_total":564102621}"#);
        let shared = vec![(70, InstallState::Queued, counts(0, 0))];
        let got = follow.look(shared.clone(), Some(&updating), None, NOT_UPDATING, 4.0);
        assert_eq!(states(&got), [("70", InstallState::Queued, None), ("60", UNKNOWN, Some(8000))]);
        let installing = overview(60, "Installing", "");
        let got = follow.look(shared, Some(&installing), None, NOT_UPDATING, 14.0);
        assert_eq!(got.len(), 2);
        // Steam has moved on: done.
        let idle = overview(0, "None", "");
        assert_eq!(follow.look(Vec::new(), Some(&idle), None, NOT_UPDATING, 16.0), []);
    }

    /// After `PauseAppUpdate` the real overview reads `update_appid: 0, update_state: "None", paused: false` (TV PC,
    /// 2026-11-17): paused is the manifest's word, never the overview's.
    #[test]
    fn a_paused_download_reads_paused_by_its_manifest_whatever_the_overview_says() {
        let mut follow = Follow::default();
        let found = vec![(60, InstallState::Downloading { done: 100, total: 5000 }, counts(100, 5000))];
        follow.look(found, Some(&overview(60, "Downloading", "")), None, NOT_UPDATING, 0.0);
        let after_pause = Overview::parse(r#"{"update_appid":0,"update_state":"None","paused":false}"#).unwrap();
        let paused = vec![(60, InstallState::Paused { done: 100, total: 5000 }, counts(100, 5000))];
        let got = follow.look(paused, Some(&after_pause), None, NOT_UPDATING, 1.0);
        assert_eq!(states(&got), [("60", InstallState::Paused { done: 100, total: 5000 }, None)]);
    }

    #[test]
    fn without_the_overview_an_app_is_held_while_content_that_turned_up_just_after_it_is_under_way() {
        let mut follow = Follow::default();
        let found = vec![(60, InstallState::Downloading { done: 0, total: 5000 }, counts(0, 5000))];
        follow.look(found, None, None, NOT_UPDATING, 0.0);
        let shared = vec![(70, InstallState::Queued, counts(0, 0))];
        let got = follow.look(shared.clone(), None, None, NOT_UPDATING, 4.0);
        assert_eq!(states(&got), [("70", InstallState::Queued, None), ("60", UNKNOWN, None)]);
        assert_eq!(follow.look(shared, None, None, NOT_UPDATING, 10.0).len(), 2);
        assert_eq!(follow.look(Vec::new(), None, None, NOT_UPDATING, 15.0), []);
    }

    #[test]
    fn an_app_whose_workshop_items_are_still_coming_is_held_with_no_overview_until_they_are_in() {
        let mut follow = Follow::default();
        let found = vec![(4000, InstallState::Downloading { done: 0, total: 5000 }, counts(0, 5000))];
        follow.look(found, None, None, NOT_UPDATING, 0.0);
        let got = follow.look_with_workshop(Vec::new(), None, None, NOT_UPDATING, &|appid| appid == 4000, 100.0);
        assert_eq!(states(&got), [("4000", UNKNOWN, None)]);
        assert!(follow.held(4000));
        assert_eq!(follow.look_with_workshop(Vec::new(), None, None, NOT_UPDATING, &|_| false, 101.0), []);
        assert!(!follow.held(4000));
    }

    #[test]
    fn an_app_is_done_when_its_manifest_says_so_and_nothing_says_steam_is_still_at_it() {
        let mut follow = Follow::default();
        // A download that was under way long before this one is no content of its: not held for it.
        follow.look(vec![(70, InstallState::Queued, counts(0, 0))], None, None, NOT_UPDATING, 0.0);
        let both = vec![
            (70, InstallState::Queued, counts(0, 0)),
            (60, InstallState::Downloading { done: 0, total: 5000 }, counts(0, 5000)),
        ];
        follow.look(both, None, None, NOT_UPDATING, 100.0);
        let got = follow.look(vec![(70, InstallState::Queued, counts(0, 0))], None, None, NOT_UPDATING, 104.0);
        assert_eq!(states(&got), [("70", InstallState::Queued, None)]);
        // The registry's word holds it, as long as it lasts.
        let mut follow = Follow::default();
        let found = vec![(60, InstallState::Downloading { done: 0, total: 5000 }, counts(0, 5000))];
        follow.look(found, None, None, NOT_UPDATING, 0.0);
        assert_eq!(states(&follow.look(Vec::new(), None, None, &|appid| appid == 60, 2.0)), [("60", UNKNOWN, None)]);
        assert_eq!(follow.look(Vec::new(), None, None, NOT_UPDATING, 3.0), []);
    }
}
