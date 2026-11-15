//! `crop <in.png> <out.png> <x> <y> <w> <h> [scale]`: a part of a screenshot, enlarged, to look at closely.
use image::imageops::FilterType;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 6 {
        eprintln!("usage: crop <in.png> <out.png> <x> <y> <w> <h> [scale]");
        std::process::exit(2);
    }
    let number = |i: usize| args[i].parse::<u32>().unwrap_or_else(|e| panic!("{}: {e}", args[i]));
    let image = image::open(&args[0]).unwrap_or_else(|e| panic!("{}: {e}", args[0])).into_rgba8();
    let (x, y) = (number(2).min(image.width() - 1), number(3).min(image.height() - 1));
    let (w, h) = (number(4).min(image.width() - x), number(5).min(image.height() - y));
    let scale = args.get(6).map_or(1, |_| number(6)).max(1);
    let part = image::imageops::crop_imm(&image, x, y, w, h).to_image();
    let part = image::imageops::resize(&part, w * scale, h * scale, FilterType::Nearest);
    part.save(&args[1]).unwrap_or_else(|e| panic!("{}: {e}", args[1]));
}
