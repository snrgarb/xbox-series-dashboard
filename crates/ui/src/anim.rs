//! Animation that lives between frames: a value per id, moving toward the target the screen names each frame.
//! A screen never starts or stops an animation; it says where a thing should be and draws it where it is.
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ease {
    Linear,
    /// Fast, then settling: WPF's CubicEase EaseOut.
    CubicOut,
    /// WPF's QuadraticEase EaseOut.
    QuadOut,
    /// WPF's SineEase EaseInOut.
    SineInOut,
}

impl Ease {
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::CubicOut => 1.0 - (1.0 - t).powi(3),
            Ease::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
            Ease::SineInOut => 0.5 - 0.5 * (std::f32::consts::PI * t).cos(),
        }
    }
}

struct Value {
    now: f32,
    from: f32,
    to: f32,
    /// Seconds since `to` last changed.
    age: f32,
    seen: u64,
}

/// An id for an animated value: any hashable thing, usually a name and an index.
pub fn id(key: impl Hash) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Every animated value, and the frame's clock.
#[derive(Default)]
pub struct Store {
    values: HashMap<u64, Value>,
    frame: u64,
    dt: f32,
    time: f64,
    /// Every animation ends at once: a screenshot's frame, with nothing mid-flight.
    settle: bool,
    /// Settings' Reduce motion: movement is instant, fades stay.
    pub reduce_motion: bool,
    /// Whether anything moved this frame, or asked for another.
    animating: bool,
}

impl Store {
    /// Starts a frame `dt` seconds after the last.
    pub fn begin(&mut self, dt: f32) {
        self.frame += 1;
        self.dt = dt;
        self.time += dt as f64;
        self.animating = false;
    }

    /// Ends the frame: values nothing asked for this frame are forgotten, so a thing that comes back starts afresh.
    pub fn end(&mut self) {
        let frame = self.frame;
        self.values.retain(|_, v| v.seen == frame);
    }

    /// Until told otherwise, every animation is at its end as soon as it is asked for.
    pub fn set_settle(&mut self, on: bool) {
        self.settle = on;
    }

    pub fn dt(&self) -> f32 {
        self.dt
    }

    /// Seconds since the first frame.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Fixes the clock that endless animations (the spinner, the swirl) read: a screenshot's is the same every run.
    pub fn set_time(&mut self, time: f64) {
        self.time = time;
    }

    /// Whether another frame is wanted without any input.
    pub fn animating(&self) -> bool {
        self.animating
    }

    /// Asks for another frame: something endless is on screen.
    pub fn keep_animating(&mut self) {
        self.animating = true;
    }

    fn value(&mut self, id: u64, target: f32) -> &mut Value {
        let frame = self.frame;
        let value = self.values.entry(id).or_insert(Value { now: target, from: target, to: target, age: 0.0, seen: 0 });
        if value.to != target {
            value.from = value.now;
            value.to = target;
            value.age = 0.0;
        }
        value.seen = frame;
        value
    }

    /// The value under `id` on its way to `target`, taking `seconds` from wherever it was when the target changed.
    /// The first time an id is asked for it is at its target already.
    pub fn tween(&mut self, id: u64, target: f32, seconds: f32, ease: Ease) -> f32 {
        let (dt, settle) = (self.dt, self.settle);
        let value = self.value(id, target);
        if value.now != value.to {
            value.age += dt;
            let t = if settle || seconds <= 0.0 { 1.0 } else { value.age / seconds };
            value.now = if t >= 1.0 { value.to } else { value.from + (value.to - value.from) * ease.at(t) };
        }
        let (now, moving) = (value.now, value.now != value.to);
        self.animating |= moving;
        now
    }

    /// As `tween`, but the first time the id is asked for it starts at `from` rather than at its target: a fade-in.
    pub fn tween_from(&mut self, id: u64, from: f32, target: f32, seconds: f32, ease: Ease) -> f32 {
        if !self.values.contains_key(&id) {
            self.value(id, from);
        }
        self.tween(id, target, seconds, ease)
    }

    /// As `tween`, but a movement: instant under Reduce motion.
    pub fn slide(&mut self, id: u64, target: f32, seconds: f32, ease: Ease) -> f32 {
        let seconds = if self.reduce_motion { 0.0 } else { seconds };
        self.tween(id, target, seconds, ease)
    }

    /// As `slide`, but the first time the id is asked for it starts at `from`: a panel sliding in.
    pub fn slide_from(&mut self, id: u64, from: f32, target: f32, seconds: f32, ease: Ease) -> f32 {
        let seconds = if self.reduce_motion { 0.0 } else { seconds };
        self.tween_from(id, from, target, seconds, ease)
    }

    /// Forgets every value: what is drawn next starts afresh. For a window that was hidden mid-animation.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// The value under `id` closing on `target` exponentially, by 63% every `tau` seconds: no fixed duration, and
    /// a new target bends its path without a jolt. Instant under Reduce motion.
    pub fn approach(&mut self, id: u64, target: f32, tau: f32) -> f32 {
        let (dt, instant) = (self.dt, self.settle || self.reduce_motion);
        let value = self.value(id, target);
        if value.now != value.to {
            let step = if instant { 1.0 } else { 1.0 - (-dt / tau).exp() };
            value.now += (value.to - value.now) * step;
            if (value.to - value.now).abs() < 0.002 {
                value.now = value.to;
            }
        }
        let (now, moving) = (value.now, value.now != value.to);
        self.animating |= moving;
        now
    }

    /// Puts the value under `id` at `value` at once, wherever it was going.
    pub fn jump(&mut self, id: u64, value: f32) {
        let v = self.value(id, value);
        v.now = value;
        v.from = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tween_starts_at_its_first_target_and_takes_its_time_to_the_next() {
        let mut store = Store::default();
        store.begin(0.016);
        assert_eq!(store.tween(1, 0.0, 0.3, Ease::Linear), 0.0);
        store.end();
        store.begin(0.1);
        assert!((store.tween(1, 1.0, 0.3, Ease::Linear) - 1.0 / 3.0).abs() < 1e-5);
        assert!(store.animating());
        store.end();
        store.begin(0.25);
        assert_eq!(store.tween(1, 1.0, 0.3, Ease::Linear), 1.0);
        assert!(!store.animating());
    }

    #[test]
    fn a_new_target_mid_flight_starts_from_where_it_is() {
        let mut store = Store::default();
        store.begin(0.0);
        store.tween(1, 0.0, 1.0, Ease::Linear);
        store.end();
        store.begin(0.5);
        assert_eq!(store.tween(1, 1.0, 1.0, Ease::Linear), 0.5);
        store.end();
        store.begin(0.5);
        assert_eq!(store.tween(1, 0.0, 1.0, Ease::Linear), 0.25);
    }

    #[test]
    fn settling_ends_everything_at_once() {
        let mut store = Store::default();
        store.begin(0.0);
        store.tween(1, 0.0, 1.0, Ease::CubicOut);
        store.approach(2, 0.0, 0.05);
        store.end();
        store.set_settle(true);
        store.begin(0.0);
        assert_eq!(store.tween(1, 1.0, 1.0, Ease::CubicOut), 1.0);
        assert_eq!(store.approach(2, 1.0, 0.05), 1.0);
    }

    #[test]
    fn a_value_nothing_asks_for_is_forgotten() {
        let mut store = Store::default();
        store.begin(0.0);
        store.tween(1, 0.0, 1.0, Ease::Linear);
        store.end();
        store.begin(0.1);
        store.end();
        store.begin(0.1);
        // Back, and at its target at once rather than animating from the old value.
        assert_eq!(store.tween(1, 1.0, 1.0, Ease::Linear), 1.0);
    }

    #[test]
    fn approach_closes_by_its_time_constant() {
        let mut store = Store::default();
        store.begin(0.0);
        store.approach(1, 0.0, 0.05);
        store.end();
        store.begin(0.05);
        let v = store.approach(1, 1.0, 0.05);
        assert!((v - (1.0 - (-1.0f32).exp())).abs() < 1e-5);
    }
}
