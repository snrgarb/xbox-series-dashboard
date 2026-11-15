//! Test support for screenshot tests: a rendered image is compared with its committed reference, within a tolerance
//! wide enough for another version of the software rasteriser or of the font, and narrow enough to catch a moved,
//! missing or recoloured element.
//!
//! `SHOTS=bless cargo test` writes the references instead of comparing. A failed comparison leaves the image it got and
//! a picture of where the two differ under `target/shots/`, which CI uploads.
use std::path::{Path, PathBuf};

use image::RgbaImage;

/// A channel may differ by this much before its pixel counts as different: antialiasing and rounding.
const CHANNEL_TOLERANCE: u8 = 24;
/// The share of pixels that may differ: glyph edges, which move with the font's and rasteriser's version.
const PIXEL_TOLERANCE: f64 = 0.004;

/// How two images of the same size differ.
pub struct Difference {
    /// The share of pixels with a channel further apart than the tolerance, 0 to 1.
    pub share: f64,
    /// The reference greyed, its differing pixels in red.
    pub picture: RgbaImage,
}

pub fn difference(got: &RgbaImage, reference: &RgbaImage) -> Difference {
    assert_eq!(got.dimensions(), reference.dimensions(), "the images differ in size");
    let mut picture = RgbaImage::new(got.width(), got.height());
    let mut differing = 0u64;
    for ((a, b), out) in got.pixels().zip(reference.pixels()).zip(picture.pixels_mut()) {
        if a.0.iter().zip(b.0).any(|(&x, y)| x.abs_diff(y) > CHANNEL_TOLERANCE) {
            differing += 1;
            *out = image::Rgba([255, 0, 0, 255]);
        } else {
            let grey = ((b.0[0] as u32 + b.0[1] as u32 + b.0[2] as u32) / 6) as u8;
            *out = image::Rgba([grey, grey, grey, 255]);
        }
    }
    Difference { share: differing as f64 / (got.width() as f64 * got.height() as f64), picture }
}

fn workspace_target() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// Checks the image at `got` against `reference_dir/<name>.png`, or with `SHOTS=bless` set makes it the reference.
/// Panics, naming the files to look at, when they differ by more than the tolerance or there is no reference.
pub fn check(name: &str, got: &Path, reference_dir: &Path) {
    let reference = reference_dir.join(format!("{name}.png"));
    if std::env::var("SHOTS").is_ok_and(|v| v == "bless") {
        std::fs::create_dir_all(reference_dir).expect("create the reference folder");
        std::fs::copy(got, &reference).expect("write the reference");
        eprintln!("shots: blessed {}", reference.display());
        return;
    }
    let got_image = image::open(got).unwrap_or_else(|e| panic!("{}: {e}", got.display())).into_rgba8();
    let out = workspace_target().join("shots");
    std::fs::create_dir_all(&out).expect("create target/shots");
    let kept = out.join(format!("{name}.got.png"));
    let fail = |why: String| -> ! {
        got_image.save(&kept).expect("keep the image");
        panic!(
            "{name}: {why}\n  got:       {}\n  reference: {}\n  to accept it: SHOTS=bless cargo test",
            kept.display(),
            reference.display()
        );
    };
    let Ok(reference_image) = image::open(&reference) else { fail("no reference image".into()) };
    let reference_image = reference_image.into_rgba8();
    if got_image.dimensions() != reference_image.dimensions() {
        fail(format!("{:?} against the reference's {:?}", got_image.dimensions(), reference_image.dimensions()));
    }
    let difference = difference(&got_image, &reference_image);
    if difference.share > PIXEL_TOLERANCE {
        let picture = out.join(format!("{name}.diff.png"));
        difference.picture.save(&picture).expect("keep the difference");
        fail(format!("{:.2}% of pixels differ (see {})", difference.share * 100.0, picture.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(colour: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(100, 100, image::Rgba(colour))
    }

    #[test]
    fn a_shade_apart_is_the_same() {
        let d = difference(&flat([100, 100, 100, 255]), &flat([110, 95, 100, 255]));
        assert_eq!(d.share, 0.0);
    }

    #[test]
    fn a_recoloured_block_differs() {
        let mut got = flat([0, 0, 0, 255]);
        for y in 0..10 {
            for x in 0..10 {
                got.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let d = difference(&got, &flat([0, 0, 0, 255]));
        assert_eq!(d.share, 0.01);
        assert_eq!(d.picture.get_pixel(0, 0).0, [255, 0, 0, 255]);
    }
}
