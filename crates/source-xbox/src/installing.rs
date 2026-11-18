//! The installs the Dashboard has asked the Store's queue for (#173), followed on one thread that looks at the
//! queue every second while any is under way, for the Queue (`Source::installs` reads what it last saw and asks
//! nothing). An install is over once the Store says it completed (the next read of Xbox finds the game installed),
//! was cancelled, failed, or left the queue: the Queue then drops it as a store's dropped install. Installs the user
//! started in the Store or the Xbox app are not the Dashboard's to follow: only its own are reported.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::{Install, InstallState};

use crate::store_queue::{Stage, StoreQueue, EVERY};

/// How long an install asked for may be missing from the queue before it is taken as gone: the Store takes a moment
/// to list what it was given.
const GRACE: Duration = Duration::from_secs(20);

/// How long after the last install ends that the installed games are still watched (`Source::installed_now`).
pub const WATCHED_AFTER: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
struct Job {
    /// The game's `source_id`, as the Queue knows it.
    source_id: String,
    name: String,
    state: InstallState,
    asked: Instant,
    /// Whether the queue has listed it yet.
    seen: bool,
}

/// What is under way, by product id.
#[derive(Clone, Default)]
pub struct Jobs {
    jobs: Arc<Mutex<HashMap<String, Job>>>,
    following: Arc<AtomicBool>,
    last_end: Arc<Mutex<Option<Instant>>>,
}

impl Jobs {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Job>> {
        self.jobs.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The Queue's word on each.
    pub fn installs(&self) -> Vec<Install> {
        let mut installs: Vec<Install> =
            self.lock().values().map(|job| Install::new(job.source_id.clone(), job.state)).collect();
        installs.sort_by(|a, b| a.source_id.cmp(&b.source_id));
        installs
    }

    pub fn has(&self, product: &str) -> bool {
        self.lock().contains_key(product)
    }

    pub fn paused(&self, product: &str) -> bool {
        self.lock().get(product).is_some_and(|job| job.state.paused())
    }

    /// Whether an install is under way, or one ended within [`WATCHED_AFTER`].
    pub fn lately(&self) -> bool {
        !self.lock().is_empty()
            || self.last_end.lock().unwrap_or_else(|p| p.into_inner()).is_some_and(|at| at.elapsed() < WATCHED_AFTER)
    }

    /// Takes on the product's install, Queued until the Store says more. False where it is under way already.
    pub fn begin(&self, product: &str, source_id: &str, name: &str) -> bool {
        let mut jobs = self.lock();
        if jobs.contains_key(product) {
            return false;
        }
        let job = Job {
            source_id: source_id.into(),
            name: name.into(),
            state: InstallState::Queued,
            asked: Instant::now(),
            seen: false,
        };
        jobs.insert(product.into(), job);
        true
    }

    /// Drops it: the Store did not take it, or it ended.
    pub fn end(&self, product: &str) {
        self.lock().remove(product);
        self.watch();
    }

    /// Has the installed games watched for [`WATCHED_AFTER`] from now: an install ended, or an uninstall asked.
    pub fn watch(&self) {
        *self.last_end.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
    }

    /// The bytes done and to do the queue last said of it.
    pub fn installs_bytes(&self, product: &str) -> (u64, u64) {
        self.lock().get(product).and_then(|job| job.state.bytes()).unwrap_or((0, 0))
    }

    /// Sets the state at once, as the user's own Pause or Resume asks, before the queue next says it.
    pub fn set(&self, product: &str, state: InstallState) {
        if let Some(job) = self.lock().get_mut(product) {
            job.state = state;
        }
    }

    /// Follows the queue on a thread of its own while anything is under way; one thread, however many installs.
    pub fn follow(&self, queue: Arc<dyn StoreQueue>) {
        if self.following.swap(true, Ordering::SeqCst) {
            return;
        }
        let jobs = self.clone();
        std::thread::spawn(move || {
            loop {
                if jobs.lock().is_empty() {
                    jobs.following.store(false, Ordering::SeqCst);
                    // Taken again if one began between the look and the store.
                    if jobs.lock().is_empty() || jobs.following.swap(true, Ordering::SeqCst) {
                        return;
                    }
                }
                match queue.items() {
                    Ok(items) => jobs.look(&items),
                    Err(why) => log(&format!("the queue could not be read ({why})")),
                }
                std::thread::sleep(EVERY);
            }
        });
    }

    /// Squares the jobs with what the queue lists now, saying in the log each change of stage.
    pub fn look(&self, items: &[crate::store_queue::Item]) {
        let mut ended = Vec::new();
        let mut jobs = self.lock();
        for (product, job) in jobs.iter_mut() {
            let Some(item) = items.iter().find(|item| item.product.eq_ignore_ascii_case(product)) else {
                if job.seen || job.asked.elapsed() > GRACE {
                    log(&format!("{product}: {} left the Store's queue", job.name));
                    ended.push(product.clone());
                }
                continue;
            };
            job.seen = true;
            match item.stage() {
                Stage::Under(state) => {
                    if std::mem::discriminant(&state) != std::mem::discriminant(&job.state) {
                        let percent = state.percent().map_or(String::new(), |p| format!(", {p}%"));
                        log(&format!("{product}: {} is {} ({}{percent})", job.name, state.status(), item.state));
                    }
                    job.state = state;
                }
                Stage::Completed => {
                    log(&format!("{product}: {} installed: the Store says complete", job.name));
                    ended.push(product.clone());
                }
                Stage::Canceled => {
                    log(&format!("{product}: {} cancelled in the Store's queue", job.name));
                    ended.push(product.clone());
                }
                Stage::Failed => {
                    log(&format!("{product}: {} failed in the Store's queue (0x{:08X})", job.name, item.error));
                    ended.push(product.clone());
                }
            }
        }
        drop(jobs);
        for product in ended {
            self.end(&product);
        }
    }
}

fn log(line: &str) {
    eprintln!("xbox: {line}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store_queue::Item;

    fn item(product: &str, state: &str, done: u64) -> Item {
        Item {
            product: product.into(),
            family: String::new(),
            state: state.into(),
            percent: 0.0,
            done,
            total: 100,
            error: 0,
        }
    }

    #[test]
    fn a_job_follows_its_item_and_ends_when_the_store_is_done_with_it() {
        let jobs = Jobs::default();
        assert!(jobs.begin("9ZZZGPASS101", "MadeUp.Saltmarsh_0", "Saltmarsh Run"));
        assert!(!jobs.begin("9ZZZGPASS101", "MadeUp.Saltmarsh_0", "Saltmarsh Run"), "under way already");
        assert_eq!(jobs.installs(), [Install::new("MadeUp.Saltmarsh_0", InstallState::Queued)]);
        // Not listed yet, within the grace: kept.
        jobs.look(&[]);
        assert!(jobs.has("9ZZZGPASS101"));
        jobs.look(&[item("9ZZZGPASS101", "Downloading", 40), item("9ZZZOTHER001", "Downloading", 1)]);
        assert_eq!(
            jobs.installs(),
            [Install::new("MadeUp.Saltmarsh_0", InstallState::Downloading { done: 40, total: 100 })]
        );
        jobs.look(&[item("9ZZZGPASS101", "Paused", 50)]);
        assert!(jobs.paused("9ZZZGPASS101"));
        jobs.look(&[item("9ZZZGPASS101", "Completed", 100)]);
        assert!(jobs.installs().is_empty());
        assert!(jobs.lately(), "watched a while after");

        // Seen once and then gone from the queue: ended.
        jobs.begin("9ZZZGPASS102", "MadeUp.Ember_0", "Ember Orchard");
        jobs.look(&[item("9ZZZGPASS102", "Pending", 0)]);
        jobs.look(&[]);
        assert!(!jobs.has("9ZZZGPASS102"));
    }
}
