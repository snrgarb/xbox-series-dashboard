//! The page's AudioContexts as Chromium's DevTools `WebAudio` domain reports them: created, changed (state) and
//! destroyed events, and `WebAudio.getRealtimeData` for each one's clock and render load. Independent of any script
//! in the page (the first TV run's in-page hooks never ran).
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{json, Value};

use super::host::Host;
use crate::say;

#[derive(Clone, Default)]
struct Context {
    state: String,
    realtime: bool,
}

pub struct WebAudio {
    contexts: Rc<RefCell<BTreeMap<String, Context>>>,
    created: Rc<RefCell<u32>>,
}

/// One reading: realtime contexts, how many running, their clocks summed, the worst render capacity.
pub struct Audio {
    pub count: usize,
    pub running: usize,
    pub clock: f64,
    pub capacity: f64,
    pub created: u32,
}

impl WebAudio {
    pub fn start(host: &Host) -> WebAudio {
        let contexts: Rc<RefCell<BTreeMap<String, Context>>> = Rc::default();
        let created = Rc::new(RefCell::new(0));
        let update = |contexts: &Rc<RefCell<BTreeMap<String, Context>>>, created: Option<&Rc<RefCell<u32>>>| {
            let contexts = contexts.clone();
            let created = created.cloned();
            Box::new(move |v: Value| {
                let c = &v["context"];
                let Some(id) = c["contextId"].as_str() else { return };
                if let Some(n) = &created {
                    *n.borrow_mut() += 1;
                }
                contexts.borrow_mut().insert(
                    id.to_string(),
                    Context {
                        state: c["contextState"].as_str().unwrap_or("?").to_string(),
                        realtime: c["contextType"] == "realtime",
                    },
                );
            }) as Box<dyn Fn(Value)>
        };
        let mut ok = host.on_event("WebAudio.contextCreated", update(&contexts, Some(&created))).is_ok();
        ok &= host.on_event("WebAudio.contextChanged", update(&contexts, None)).is_ok();
        let gone = contexts.clone();
        ok &= host
            .on_event(
                "WebAudio.contextWillBeDestroyed",
                Box::new(move |v| {
                    if let Some(id) = v["contextId"].as_str() {
                        gone.borrow_mut().remove(id);
                    }
                }),
            )
            .is_ok();
        match host.cdp("WebAudio.enable", json!({})) {
            Ok(_) if ok => say!("webaudio: DevTools WebAudio enabled: the page's AudioContexts are followed"),
            Ok(_) => say!("webaudio: enabled, but an event could not be listened to"),
            Err(e) => say!("webaudio: DevTools WebAudio refused: {e}"),
        }
        WebAudio { contexts, created }
    }

    pub fn read(&self, host: &Host) -> Audio {
        let list: Vec<(String, Context)> = self.contexts.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let mut a = Audio { count: 0, running: 0, clock: 0.0, capacity: 0.0, created: *self.created.borrow() };
        for (id, c) in list.iter().filter(|(_, c)| c.realtime) {
            a.count += 1;
            if c.state == "running" {
                a.running += 1;
            }
            if let Ok(r) = host.cdp("WebAudio.getRealtimeData", json!({ "contextId": id })) {
                a.clock += r["realtimeData"]["currentTime"].as_f64().unwrap_or(0.0);
                a.capacity = a.capacity.max(r["realtimeData"]["renderCapacity"].as_f64().unwrap_or(0.0));
            }
        }
        a
    }
}
