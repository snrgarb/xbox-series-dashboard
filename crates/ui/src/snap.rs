//! Pictures of part of a frame, taken once and kept: what a panel shows of the screen it was opened over
//! (`Ui::snapshot`). They live with the animated values, in the `Store`, and by the same rule: one a frame does not
//! ask for is forgotten, and its texture freed.
use std::collections::HashMap;

use render::{Frame, TexId};

/// A picture that was taken: a texture, and its size in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub tex: TexId,
    pub size: (u32, u32),
}

/// A picture still to be taken: the frame to draw, for an output of `output` pixels, and how it is shrunk and
/// blurred into the texture `tex`. The `Host` takes it before it draws the frame that asked for it.
pub(crate) struct Job {
    pub tex: TexId,
    pub frame: Frame,
    pub output: (u32, u32),
    pub shrink: u32,
    pub blur: f32,
}

#[derive(Default)]
pub(crate) struct Snaps {
    /// Each picture, and the frame that last asked for it.
    taken: HashMap<u64, (Snapshot, u64)>,
    /// A texture with nothing in it yet, for the next picture: made by the `Host`, which has the renderer.
    spare: Option<TexId>,
    jobs: Vec<Job>,
    /// Textures of pictures that were forgotten, for the `Host` to free.
    dropped: Vec<TexId>,
}

impl Snaps {
    /// The picture under `id`, asked for in frame `now`.
    pub fn taken(&mut self, id: u64, now: u64) -> Option<Snapshot> {
        let (snapshot, seen) = self.taken.get_mut(&id)?;
        *seen = now;
        Some(*snapshot)
    }

    pub fn wants_spare(&self) -> bool {
        self.spare.is_none()
    }

    pub fn set_spare(&mut self, tex: TexId) {
        self.spare = Some(tex);
    }

    pub fn take_spare(&mut self) -> Option<TexId> {
        self.spare.take()
    }

    /// Keeps `snapshot` under `id` from frame `now`, to be drawn by `job`.
    pub fn take(&mut self, id: u64, now: u64, snapshot: Snapshot, job: Job) -> Snapshot {
        self.taken.insert(id, (snapshot, now));
        self.jobs.push(job);
        snapshot
    }

    /// Frame `frame` has been declared: what it did not ask for is forgotten.
    pub fn end(&mut self, frame: u64) {
        let dropped = &mut self.dropped;
        self.taken.retain(|_, (snapshot, seen)| {
            let kept = *seen == frame;
            if !kept {
                dropped.push(snapshot.tex);
            }
            kept
        });
    }

    /// Forgets every picture.
    pub fn clear(&mut self) {
        self.dropped.extend(self.taken.drain().map(|(_, (snapshot, _))| snapshot.tex));
    }

    pub fn take_jobs(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.jobs)
    }

    pub fn take_dropped(&mut self) -> Vec<TexId> {
        std::mem::take(&mut self.dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(tex: TexId) -> Job {
        Job { tex, frame: Frame::default(), output: (1920, 1080), shrink: 8, blur: 2.0 }
    }

    #[test]
    fn a_picture_is_kept_while_it_is_asked_for_and_its_texture_freed_after() {
        let mut snaps = Snaps::default();
        assert!(snaps.wants_spare() && snaps.taken(7, 1).is_none());
        snaps.set_spare(render::WHITE);
        let tex = snaps.take_spare().unwrap();
        // The spare is used: a second new picture in the same frame has none.
        assert!(snaps.take_spare().is_none() && snaps.wants_spare());
        let snapshot = snaps.take(7, 1, Snapshot { tex, size: (240, 135) }, job(tex));
        snaps.end(1);
        assert_eq!(snaps.take_jobs().len(), 1);
        assert!(snaps.take_dropped().is_empty());
        // Asked for again in the next frame, it is the same picture, and is not taken again.
        assert_eq!(snaps.taken(7, 2), Some(snapshot));
        snaps.end(2);
        assert!(snaps.take_jobs().is_empty() && snaps.take_dropped().is_empty());
        // A frame that does not ask for it lets it go.
        snaps.end(3);
        assert_eq!(snaps.take_dropped(), [tex]);
        assert!(snaps.taken(7, 4).is_none());
    }
}
