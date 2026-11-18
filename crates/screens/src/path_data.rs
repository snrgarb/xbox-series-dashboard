//! Path data as XAML and SVG write it ("M 10,2 L 34,2 C 1,2 3,4 5,6 A 3,3 0 0 1 8,9 Z"), read into a tiny-skia
//! path, so a drawing carried over from the Bundle's XAML or from Simple Icons keeps its numbers as they are written
//! there. Every command SVG has, the arc circular only: M, L, H, V, C, S, Q, T, A and Z, absolute in upper case and
//! relative in lower. A command's letter may be left out when it repeats, and an arc's two flags may run into what
//! follows them ("a3 3 0 01-2.5 1"), as SVG minifiers write them. A circular arc becomes cubics of a quarter turn at
//! most; tiny-skia flattens and strokes them, anti-aliased and with the joins it is asked for.
//!
//! The marks of other companies the Dashboard shows are kept here as their path data, each with where it is from
//! (NOTICE.md at the repository's root says whose each is).
use tiny_skia as sk;

/// The path `data` draws. Panics on data it cannot read: path data is written in the code, never read from a file.
pub fn parse(data: &str) -> sk::Path {
    let mut tokens = Tokens { rest: data, pending: None };
    let mut path = sk::PathBuilder::new();
    let mut command = None;
    let mut at = (0.0f32, 0.0f32);
    // Where the figure began, where a close goes back to; and the last curve's kind and second handle, which an S or
    // a T mirrors.
    let mut start = at;
    let mut handle: Option<(char, (f32, f32))> = None;
    while let Some(token) = tokens.next() {
        let letter = match token {
            Token::Command(letter) => letter,
            Token::Number(_) => {
                tokens.back(token);
                // A repeated command; after a move, lines.
                match command {
                    Some('M') => 'L',
                    Some('m') => 'l',
                    Some(letter) => letter,
                    None => panic!("path data starts with a number: {data}"),
                }
            }
        };
        command = Some(letter);
        let base = if letter.is_ascii_lowercase() { at } else { (0.0, 0.0) };
        let mirrored = |kind: char| match handle {
            Some((last, (hx, hy))) if last == kind => (2.0 * at.0 - hx, 2.0 * at.1 - hy),
            _ => at,
        };
        let mut curve = None;
        match letter.to_ascii_uppercase() {
            'M' => {
                at = tokens.point(base, data);
                start = at;
                path.move_to(at.0, at.1);
            }
            'L' => {
                at = tokens.point(base, data);
                path.line_to(at.0, at.1);
            }
            'H' => {
                at.0 = base.0 + tokens.must(data);
                path.line_to(at.0, at.1);
            }
            'V' => {
                at.1 = base.1 + tokens.must(data);
                path.line_to(at.0, at.1);
            }
            'C' | 'S' => {
                let one = if letter.eq_ignore_ascii_case(&'C') { tokens.point(base, data) } else { mirrored('C') };
                let (two, to) = (tokens.point(base, data), tokens.point(base, data));
                path.cubic_to(one.0, one.1, two.0, two.1, to.0, to.1);
                (at, curve) = (to, Some(('C', two)));
            }
            'Q' | 'T' => {
                let one = if letter.eq_ignore_ascii_case(&'Q') { tokens.point(base, data) } else { mirrored('Q') };
                let to = tokens.point(base, data);
                path.quad_to(one.0, one.1, to.0, to.1);
                (at, curve) = (to, Some(('Q', one)));
            }
            'A' => {
                let (rx, ry, _turn) = (tokens.must(data), tokens.must(data), tokens.must(data));
                let (large, sweep) = (tokens.flag(data), tokens.flag(data));
                let to = tokens.point(base, data);
                assert!((rx - ry).abs() < 1e-3, "only circular arcs: {data}");
                arc(&mut path, at, rx, large, sweep, to);
                at = to;
            }
            'Z' => {
                path.close();
                at = start;
                command = None;
            }
            _ => panic!("path data's {letter} is not read here: {data}"),
        }
        handle = curve;
    }
    path.finish().unwrap_or_else(|| panic!("path data draws nothing: {data}"))
}

/// The marks of the stores, of the console and of Discord, white on the Dashboard, as the Bundle draws its three
/// (Steam's, the Xbox sphere and Discord's) and Simple Icons (CC0 1.0, simpleicons.org) the rest. Each is its owner's
/// trademark.
pub mod marks {
    /// Steam's mark, Valve's: the Bundle's own path (`E/HomeTiles.xaml` SourceSteam), which is Simple Icons' `steam`
    /// (the same in 9.21.0 and 16.38.0). 24 x 24.
    pub const STEAM: &str = "M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 \
        .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 \
        4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 \
        0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 \
        11.999-12S18.605 0 11.979 0zM7.54 18.21l-1.473-.61c.262.543.714.999 1.314 1.25 1.297.539 2.793-.076 \
        3.332-1.375.263-.63.264-1.319.005-1.949s-.75-1.121-1.377-1.383c-.624-.26-1.29-.249-1.878-.03l1.523.63c.956.4 \
        1.409 1.5 1.009 2.455-.397.957-1.497 1.41-2.454 \
        1.012H7.54zm11.415-9.303c0-1.662-1.353-3.015-3.015-3.015-1.665 0-3.015 1.353-3.015 3.015 0 1.665 1.35 \
        3.015 3.015 3.015 1.663 0 3.015-1.35 3.015-3.015zm-5.273-.005c0-1.252 1.013-2.266 2.265-2.266 1.249 0 \
        2.266 1.014 2.266 2.266 0 1.251-1.017 2.265-2.266 2.265-1.253 0-2.265-1.014-2.265-2.265z";
    /// Epic Games' mark, Epic Games, Inc.'s: Simple Icons 16.38.0 `epicgames`. 24 x 24.
    pub const EPIC_GAMES: &str = "M3.537 0C2.165 0 1.66.506 1.66 1.879V18.44a4.262 4.262 0 \
        00.02.433c.031.3.037.59.316.92.027.033.311.245.311.245.153.075.258.13.43.2l8.335 \
        3.491c.433.199.614.276.928.27h.002c.314.006.495-.071.928-.27l8.335-3.492c.172-.07.277-.124.43-.2 0 0 \
        .284-.211.311-.243.28-.33.285-.621.316-.92a4.261 4.261 0 \
        00.02-.434V1.879c0-1.373-.506-1.88-1.878-1.88zm13.366 3.11h.68c1.138 0 1.688.553 1.688 \
        1.696v1.88h-1.374v-1.8c0-.369-.17-.54-.523-.54h-.235c-.367 0-.537.17-.537.539v5.81c0 \
        .369.17.54.537.54h.262c.353 0 .523-.171.523-.54V8.619h1.373v2.143c0 1.144-.562 1.71-1.7 \
        1.71h-.694c-1.138 0-1.7-.566-1.7-1.71V4.82c0-1.144.562-1.709 \
        1.7-1.709zm-12.186.08h3.114v1.274H6.117v2.603h1.648v1.275H6.117v2.774h1.74v1.275h-3.14zm3.816 \
        0h2.198c1.138 0 1.7.564 1.7 1.708v2.445c0 1.144-.562 1.71-1.7 1.71h-.799v3.338h-1.4zm4.53 \
        0h1.4v9.201h-1.4zm-3.13 1.235v3.392h.575c.354 0 .523-.171.523-.54V4.965c0-.368-.17-.54-.523-.54zm-3.74 \
        10.147a1.708 1.708 0 01.591.108 1.745 1.745 0 01.49.299l-.452.546a1.247 1.247 0 00-.308-.195.91.91 0 \
        00-.363-.068.658.658 0 00-.28.06.703.703 0 00-.224.163.783.783 0 00-.151.243.799.799 0 \
        00-.056.299v.008a.852.852 0 00.056.31.7.7 0 00.157.245.736.736 0 00.238.16.774.774 0 00.303.058.79.79 0 \
        00.445-.116v-.339h-.548v-.565H7.37v1.255a2.019 2.019 0 01-.524.307 1.789 1.789 0 01-.683.123 1.642 1.642 \
        0 01-.602-.107 1.46 1.46 0 01-.478-.3 1.371 1.371 0 01-.318-.455 1.438 1.438 0 01-.115-.58v-.008a1.426 \
        1.426 0 01.113-.57 1.449 1.449 0 01.312-.46 1.418 1.418 0 01.474-.309 1.58 1.58 0 01.598-.111 1.708 \
        1.708 0 01.045 0zm11.963.008a2.006 2.006 0 01.612.094 1.61 1.61 0 01.507.277l-.386.546a1.562 1.562 0 \
        00-.39-.205 1.178 1.178 0 00-.388-.07.347.347 0 00-.208.052.154.154 0 00-.07.127v.008a.158.158 0 \
        00.022.084.198.198 0 00.076.066.831.831 0 00.147.06c.062.02.14.04.236.061a3.389 3.389 0 01.43.122 1.292 \
        1.292 0 01.328.17.678.678 0 01.207.24.739.739 0 01.071.337v.008a.865.865 0 01-.081.382.82.82 0 \
        01-.229.285 1.032 1.032 0 01-.353.18 1.606 1.606 0 01-.46.061 2.16 2.16 0 01-.71-.116 1.718 1.718 0 \
        01-.593-.346l.43-.514c.277.223.578.335.9.335a.457.457 0 00.236-.05.157.157 0 00.082-.142v-.008a.15.15 0 \
        00-.02-.077.204.204 0 00-.073-.066.753.753 0 00-.143-.062 2.45 2.45 0 00-.233-.062 5.036 5.036 0 \
        01-.413-.113 1.26 1.26 0 01-.331-.16.72.72 0 01-.222-.243.73.73 0 01-.082-.36v-.008a.863.863 0 \
        01.074-.359.794.794 0 01.214-.283 1.007 1.007 0 01.34-.185 1.423 1.423 0 01.448-.066 2.006 2.006 0 \
        01.025 0zm-9.358.025h.742l1.183 2.81h-.825l-.203-.499H8.623l-.198.498h-.81zm2.197.02h.814l.663 \
        1.08.663-1.08h.814v2.79h-.766v-1.602l-.711 1.091h-.016l-.707-1.083v1.593h-.754zm3.469 \
        0h2.235v.658h-1.473v.422h1.334v.61h-1.334v.442h1.493v.658h-2.255zm-5.3.897l-.315.793h.624zm-1.145 \
        5.19h8.014l-4.09 1.348z";
    /// GOG's mark, GOG sp. z o.o.'s: Simple Icons 16.38.0 `gogdotcom`. 24 x 24.
    pub const GOG: &str = "M7.15 15.24H4.36a.4.4 0 0 0-.4.4v2c0 .21.18.4.4.4h2.8v1.32h-3.5c-.56 \
        0-1.02-.46-1.02-1.03v-3.39c0-.56.46-1.02 1.03-1.02h3.48v1.32zM8.16 11.54c0 .58-.47 1.05-1.05 \
        1.05H2.63v-1.35h3.78a.4.4 0 0 0 .4-.4V6.39a.4.4 0 0 0-.4-.4H4.39a.4.4 0 0 0-.41.4v2.02c0 \
        .23.18.4.4.4H6v1.35H3.68c-.58 0-1.05-.46-1.05-1.04V5.68c0-.57.47-1.04 1.05-1.04H7.1c.58 0 1.05.47 1.05 \
        1.04v5.86zM21.36 19.36h-1.32v-4.12h-.93a.4.4 0 0 0-.4.4v3.72h-1.33v-4.12h-.93a.4.4 0 0 \
        0-.4.4v3.72h-1.33v-4.42c0-.56.46-1.02 1.03-1.02h5.61v5.44zM21.37 11.54c0 .58-.47 1.05-1.05 \
        1.05h-4.48v-1.35h3.78a.4.4 0 0 0 .4-.4V6.39a.4.4 0 0 0-.4-.4h-2.03a.4.4 0 0 0-.4.4v2.02c0 \
        .23.18.4.4.4h1.62v1.35H16.9c-.58 0-1.05-.46-1.05-1.04V5.68c0-.57.47-1.04 1.05-1.04h3.43c.58 0 1.05.47 \
        1.05 1.04v5.86zM13.72 4.64h-3.44c-.58 0-1.04.47-1.04 1.04v3.44c0 .58.46 1.04 1.04 1.04h3.44c.57 0 \
        1.04-.46 1.04-1.04V5.68c0-.57-.47-1.04-1.04-1.04m-.3 1.75v2.02a.4.4 0 0 1-.4.4h-2.03a.4.4 0 0 \
        1-.4-.4V6.4c0-.22.17-.4.4-.4H13c.23 0 .4.18.4.4zM12.63 13.92H9.24c-.57 0-1.03.46-1.03 1.02v3.39c0 .57.46 \
        1.03 1.03 1.03h3.39c.57 0 1.03-.46 1.03-1.03v-3.39c0-.56-.46-1.02-1.03-1.02m-.3 1.72v2a.4.4 0 0 \
        1-.4.4v-.01H9.94a.4.4 0 0 1-.4-.4v-1.99c0-.22.18-.4.4-.4h2c.22 0 .4.18.4.4zM23.49 1.1a1.74 1.74 0 0 \
        0-1.24-.52H1.75A1.74 1.74 0 0 0 0 2.33v19.34a1.74 1.74 0 0 0 1.75 1.75h20.5A1.74 1.74 0 0 0 24 \
        21.67V2.33c0-.48-.2-.92-.51-1.24m0 20.58a1.23 1.23 0 0 1-1.24 1.24H1.75A1.23 1.23 0 0 1 .5 \
        21.67V2.33a1.23 1.23 0 0 1 1.24-1.24h20.5a1.24 1.24 0 0 1 1.24 1.24v19.34z";
    /// Battle.net's mark, Blizzard Entertainment's: Simple Icons 16.38.0 `battledotnet`. 24 x 24.
    pub const BATTLE_NET: &str =
        "M18.94 8.296C15.9 6.892 11.534 6 7.426 6.332c.206-1.36.714-2.308 1.548-2.508 1.148-.275 2.4.48 3.594 \
        1.854.782.102 1.71.28 2.355.429C12.747 2.013 9.828-.282 7.607.565c-1.688.644-2.553 2.97-2.448 \
        6.094-2.2.468-3.915 1.3-5.013 2.495-.056.065-.181.227-.137.305.034.058.146-.008.194-.04 1.274-.89 \
        2.904-1.373 5.027-1.676.303 3.333 1.713 7.56 4.055 \
        10.952-1.28.502-2.356.536-2.946-.087-.812-.856-.784-2.318-.19-4.04a26.764 26.764 0 0 1-.807-2.254c-2.459 \
        3.934-2.986 7.61-1.143 9.11 1.402 1.14 3.847.725 6.502-.926 1.505 1.672 3.083 2.74 4.667 \
        3.094.084.015.287.043.332-.034.034-.06-.08-.124-.131-.149-1.408-.657-2.64-1.828-3.964-3.515 2.735-1.929 \
        5.691-5.263 7.457-8.988 1.076.86 1.64 1.773 1.398 2.595-.336 1.131-1.615 1.84-3.403 2.185a27.697 27.697 \
        0 0 1-1.548 1.826c4.634.16 8.08-1.22 \
        8.458-3.565.286-1.786-1.295-3.696-4.053-5.17.696-2.139.832-4.04.346-5.588-.029-.08-.106-.27-.196-.27-.068 \
        0-.067.13-.063.187.135 1.547-.263 3.2-1.062 5.19zm-8.533 9.869c-1.96-3.145-3.09-6.849-3.082-10.594 \
        3.702-.124 7.474.748 10.714 2.627-1.743 3.269-4.385 6.1-7.633 7.966h.001z";
    /// EA's mark, Electronic Arts': Simple Icons 16.38.0 `ea`. 24 x 24.
    pub const EA: &str = "M16.635 6.162l-5.928 9.377H4.24l1.508-2.3h4.024l1.474-2.335H2.264L.79 13.239h2.156L0 \
        17.84h12.072l4.563-7.259 1.652 2.66h-1.401l-1.473 2.299h4.347l1.473 2.3H24zm-11.461.107L3.7 \
        8.604l9.52-.035 1.474-2.3z";
    /// Ubisoft's mark, Ubisoft Entertainment's: Simple Icons 16.38.0 `ubisoft`. 24 x 24.
    pub const UBISOFT: &str =
        "M23.561 11.988C23.301-.304 6.954-4.89.656 6.634c.282.206.661.477.943.672a11.747 11.747 0 00-.976 3.067 \
        11.885 11.885 0 00-.184 2.071C.439 18.818 5.621 24 12.005 24c6.385 0 11.556-5.17 \
        11.556-11.556v-.455zm-20.27 2.06c-.152 1.246-.054 1.636-.054 \
        1.788l-.282.098c-.108-.206-.37-.932-.488-1.908C2.163 10.308 4.7 6.96 8.57 6.33c3.544-.52 6.937 1.68 \
        7.728 4.758l-.282.098c-.087-.087-.228-.336-.77-.878-4.281-4.281-11.002-2.32-11.956 3.74zm11.002 \
        2.081a3.145 3.145 0 01-2.59 1.355 3.15 3.15 0 01-3.155-3.155 3.159 3.159 0 012.927-3.144c1.018-.043 \
        1.972.51 2.416 1.398a2.58 2.58 0 01-.455 2.95c.293.205.575.4.856.595zm6.58.12c-1.669 3.782-5.106 \
        5.766-8.77 5.712-7.034-.347-9.083-8.466-4.38-11.393l.207.206c-.076.108-.358.325-.791 1.182-.51 \
        1.041-.672 2.081-.607 2.732.369 5.67 8.314 6.83 11.045 1.214C21.057 8.217 11.822.401 3.626 \
        6.374l-.184-.184C5.599 2.808 9.816 1.3 13.837 2.309c6.147 1.55 9.453 7.956 7.035 13.94z";
    /// The Xbox sphere, Microsoft's: the Bundle's own path (`E/HomeTiles.xaml` SourceXbox, `E/OverlayWindow.xaml`
    /// XboxLogo), which is Simple Icons 9.21.0's `xbox`, the last release to carry it. 24 x 24, the sphere filling it.
    pub const XBOX: &str = "M4.102 21.033C6.211 22.881 8.977 24 12 24c3.026 0 5.789-1.119 7.902-2.967 \
        1.877-1.912-4.316-8.709-7.902-11.417-3.582 2.708-9.779 9.505-7.898 11.417zm11.16-14.406c2.5 2.961 7.484 \
        10.313 6.076 12.912C23.002 17.48 24 14.861 24 12.004c0-3.34-1.365-6.362-3.57-8.536 0 \
        0-.027-.022-.082-.042-.063-.022-.152-.045-.281-.045-.592 0-1.985.434-4.805 3.246zM3.654 \
        3.426c-.057.02-.082.041-.086.042C1.365 5.642 0 8.664 0 12.004c0 2.854.998 5.473 2.661 7.533-1.401-2.605 \
        3.579-9.951 6.08-12.91-2.82-2.813-4.216-3.245-4.806-3.245-.131 0-.223.021-.281.046v-.002zM12 3.551S9.055 \
        1.828 6.755 1.746c-.903-.033-1.454.295-1.521.339C7.379.646 9.659 0 11.984 0H12c2.334 0 4.605.646 6.766 \
        2.085-.068-.046-.615-.372-1.52-.339C14.946 1.828 12 3.545 12 3.545v.006z";

    /// Discord's mark, Discord Inc.'s: the Bundle's own path (`E/OverlayWindow.xaml` DiscordLogo), which is Simple
    /// Icons' `discord` written out with spaces (the same numbers in every release from 5.0.0 to 16.38.0; this is
    /// 16.38.0's). 24 wide, 18.3 high, on the 24 grid's middle.
    pub const DISCORD: &str = "M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 \
        00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 \
        0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 \
        1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 \
        00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 \
        00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 \
        00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 \
        01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 \
        1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 \
        12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 \
        1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 \
        00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 \
        15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 \
        1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 \
        0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 \
        0 1.3332-.946 2.4189-2.1568 2.4189Z";

    /// The Xbox sphere as the Bundle's idle pad draws it on the Guide button (`E/IdleCardControl.xaml`, Canvas
    /// "Guide"): Kenney's Input Prompts' `xbox_guide.svg` (CC0 1.0, kenney.nl), on a 64 grid centred on (32, 32),
    /// radius 24. Its three lower pieces, outlined, and its top, filled.
    pub const XBOX_GUIDE_PIECES: &str =
        "M51.3 45.75 Q51.65 44.05 50.6 40.7 49.55 37.45 46.45 32.2 43.3 26.95 40.05 22.85 L38.7 21.1 40.15 19.75 \
        Q42.85 17.3 44.9 16 45.75 15.5 46.8 15.1 L48.4 14.65 Q48.7 14.65 49.7 15.7 L51.6 18 Q54.5 22.05 55.6 \
        27.3 55.95 28.9 56 31.8 L55.7 36.35 Q55.1 39.45 53.9 42 L52.7 44.3 51.4 46.25 51.15 46.45 51.3 45.8 51.3 \
        45.75 M29.6 55.9 Q23.9 55.35 18.95 52.1 L16.3 50.05 Q15.65 49.3 15.65 48.2 15.65 46.05 17.9 42.4 20.2 \
        38.8 24.25 34.45 26.55 32 29.2 29.65 31.8 27.35 32.1 27.4 32.8 27.6 36.9 31.55 40.95 35.5 43 38 46.25 \
        41.95 47.65 45 49 48.05 48.25 49.5 47.7 50.6 44.8 52.35 41.9 54.1 39.3 54.9 L34.5 55.85 29.6 55.9 M12 \
        45.2 Q10.3 42.6 9.4 39.85 8.45 37.15 8.1 33.5 L8 31.2 8.45 27.65 Q8.95 24.55 10.6 21.3 12.2 18 14.35 \
        15.8 15.2 14.85 15.6 14.75 L17.1 15 Q19.55 15.85 23.75 19.6 L25.45 21.15 24.5 22.3 Q21.2 26.35 18.2 \
        31.35 15.1 36.45 13.8 39.95 13.1 41.85 12.85 43.6 12.55 45.35 12.9 46.15 L12.9 46.5 12 45.2";
    pub const XBOX_GUIDE_TOP: &str =
        "M29.95 13.85 Q28.5 13.1 26.45 12.4 24.45 11.65 23.15 11.45 L20.9 11.3 19.95 11.2 Q20.1 11 21.8 10.25 \
        24.4 9 27.4 8.35 L32.05 8 36.65 8.35 Q38.6 8.75 40.65 9.5 42.65 10.25 43.9 11.1 L44.4 11.4 43.3 11.35 \
        Q39.75 11.1 34.05 13.85 L31.95 14.8 29.95 13.85 Z";
}

/// A circular arc of radius `r` from `from` to `to` (SVG's endpoint arc, its centre worked out as SVG's
/// implementation notes F.6.5 do, with the two radii equal), as cubics of a quarter turn at most.
fn arc(path: &mut sk::PathBuilder, from: (f32, f32), r: f32, large: bool, sweep: bool, to: (f32, f32)) {
    let (x0, y0) = (from.0 as f64, from.1 as f64);
    let (x, y) = (to.0 as f64, to.1 as f64);
    if r <= 0.0 || (x0 - x).abs() + (y0 - y).abs() < 1e-9 {
        path.line_to(to.0, to.1);
        return;
    }
    let (hx, hy) = ((x0 - x) / 2.0, (y0 - y) / 2.0);
    let r = (r as f64).max((hx * hx + hy * hy).sqrt());
    let square = hx * hx + hy * hy;
    let mut coef = ((r * r - square) / square).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let (cx, cy) = (coef * hy + (x0 + x) / 2.0, -coef * hx + (y0 + y) / 2.0);
    let start = (y0 - cy).atan2(x0 - cx);
    let mut turn = (y - cy).atan2(x - cx) - start;
    use std::f64::consts::{FRAC_PI_2, TAU};
    if sweep && turn < 0.0 {
        turn += TAU;
    } else if !sweep && turn > 0.0 {
        turn -= TAU;
    }
    let pieces = (turn.abs() / FRAC_PI_2 - 1e-9).ceil().max(1.0) as usize;
    let step = turn / pieces as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan() * r;
    for piece in 0..pieces {
        let a = start + step * piece as f64;
        let b = a + step;
        let (p0x, p0y) = (cx + r * a.cos(), cy + r * a.sin());
        let (p3x, p3y) = if piece + 1 == pieces { (x, y) } else { (cx + r * b.cos(), cy + r * b.sin()) };
        let (p1x, p1y) = (p0x - k * a.sin(), p0y + k * a.cos());
        let (p2x, p2y) = (p3x + k * b.sin(), p3y - k * b.cos());
        path.cubic_to(p1x as f32, p1y as f32, p2x as f32, p2y as f32, p3x as f32, p3y as f32);
    }
}

#[derive(Clone, Copy)]
enum Token {
    Command(char),
    Number(f32),
}

struct Tokens<'a> {
    rest: &'a str,
    pending: Option<Token>,
}

impl Tokens<'_> {
    fn next(&mut self) -> Option<Token> {
        if let Some(token) = self.pending.take() {
            return Some(token);
        }
        self.rest = self.rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        let first = self.rest.chars().next()?;
        if first.is_ascii_alphabetic() && first != 'e' && first != 'E' {
            self.rest = &self.rest[1..];
            return Some(Token::Command(first));
        }
        // A number: a sign, digits and one point, an exponent.
        let bytes = self.rest.as_bytes();
        let mut end = 0;
        let mut point = false;
        while end < bytes.len() {
            let c = bytes[end];
            let sign = (c == b'-' || c == b'+') && (end == 0 || matches!(bytes[end - 1], b'e' | b'E'));
            if c.is_ascii_digit() || sign || (c == b'.' && !point) || ((c == b'e' || c == b'E') && end > 0) {
                point |= c == b'.';
                end += 1;
            } else {
                break;
            }
        }
        let (number, rest) = self.rest.split_at(end.max(1));
        self.rest = rest;
        Some(Token::Number(number.parse().unwrap_or_else(|_| panic!("path data's {number:?} is not a number"))))
    }

    fn number(&mut self) -> Option<f32> {
        match self.next()? {
            Token::Number(n) => Some(n),
            Token::Command(_) => None,
        }
    }

    /// The next number, which the command must have.
    fn must(&mut self, data: &str) -> f32 {
        self.number().unwrap_or_else(|| panic!("path data is short: {data}"))
    }

    /// The next two numbers, a point, from `base` (the current point for a relative command, else the origin).
    fn point(&mut self, base: (f32, f32), data: &str) -> (f32, f32) {
        let x = self.must(data);
        (base.0 + x, base.1 + self.must(data))
    }

    /// An arc's flag: one figure, 0 or 1, which may run straight into the next number.
    fn flag(&mut self, data: &str) -> bool {
        assert!(self.pending.is_none(), "an arc's flag comes after its numbers");
        self.rest = self.rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        let flag = self.rest.chars().next();
        self.rest = self.rest.get(1..).unwrap_or("");
        match flag {
            Some('0') => false,
            Some('1') => true,
            _ => panic!("path data's arc has no flag where one should be: {data}"),
        }
    }

    /// Puts a token back, to be the next.
    fn back(&mut self, token: Token) {
        self.pending = Some(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(path: &sk::Path) -> Vec<(f32, f32)> {
        path.points().iter().map(|p| ((p.x * 1000.0).round() / 1000.0, (p.y * 1000.0).round() / 1000.0)).collect()
    }

    #[test]
    fn reads_moves_lines_and_cubics_as_the_xaml_writes_them() {
        let path = parse("M 181.1,13.8 C 175.1,13.8 168.7,14.5 162.8,13.6 L 279.2,225.2 Z");
        assert_eq!(points(&path), [(181.1, 13.8), (175.1, 13.8), (168.7, 14.5), (162.8, 13.6), (279.2, 225.2)]);
        let kinds: Vec<&str> = path
            .segments()
            .map(|segment| match segment {
                sk::PathSegment::MoveTo(_) => "move",
                sk::PathSegment::LineTo(_) => "line",
                sk::PathSegment::QuadTo(..) => "quad",
                sk::PathSegment::CubicTo(..) => "cubic",
                sk::PathSegment::Close => "close",
            })
            .collect();
        assert_eq!(kinds, ["move", "cubic", "line", "close"]);
    }

    #[test]
    fn a_repeated_command_may_leave_its_letter_out() {
        let path = parse("M0 0 10 0 L 10 10 0 10 Q 1 2 3 4 5 6 7 8");
        assert_eq!(
            points(&path),
            [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0)]
        );
    }

    /// A quarter turn clockwise on the screen (sweep 1) from the top of a circle about (0, 10) to its right: one cubic
    /// with the circle's handles, 0.5523 of the radius long.
    #[test]
    fn a_quarter_arc_is_one_cubic_with_the_circles_handles() {
        let path = parse("M 0,0 A 10,10 0 0 1 10,10");
        assert_eq!(points(&path), [(0.0, 0.0), (5.523, 0.0), (10.0, 4.477), (10.0, 10.0)]);
        // The other way round the same two points bends the other way, about (10, 0).
        let path = parse("M 0,0 A 10,10 0 0 0 10,10");
        assert_eq!(points(&path), [(0.0, 0.0), (0.0, 5.523), (4.477, 10.0), (10.0, 10.0)]);
    }

    #[test]
    fn a_relative_command_goes_from_the_current_point_and_a_close_returns_to_the_figures_start() {
        let path = parse("M1 1h2v2l-1 1zm5 0c1 0 2 1 2 2s-1 2-2 2");
        assert_eq!(
            points(&path),
            [
                (1.0, 1.0),
                (3.0, 1.0),
                (3.0, 3.0),
                (2.0, 4.0),
                (6.0, 1.0),
                (7.0, 1.0),
                (8.0, 2.0),
                (8.0, 3.0),
                // S mirrors the last cubic's second handle about the point it reached.
                (8.0, 4.0),
                (7.0, 5.0),
                (6.0, 5.0)
            ]
        );
    }

    /// Simple Icons writes an arc's flags run into its end point: "0 01-.455 2.95" is flags 0 and 1, then the point
    /// (-.455, 2.95).
    #[test]
    fn an_arcs_flags_may_run_into_its_end_point() {
        let packed = parse("M0 0a5 5 0 01-.455 2.95");
        let spaced = parse("M0 0 A5 5 0 0 1 -.455 2.95");
        assert_eq!(points(&packed), points(&spaced));
    }

    #[test]
    fn every_mark_reads_and_keeps_to_its_box() {
        let sphere = format!("{} {}", marks::XBOX_GUIDE_PIECES, marks::XBOX_GUIDE_TOP);
        for (data, side) in [
            (marks::STEAM, 24.0),
            (marks::EPIC_GAMES, 24.0),
            (marks::GOG, 24.0),
            (marks::BATTLE_NET, 24.0),
            (marks::EA, 24.0),
            (marks::UBISOFT, 24.0),
            (marks::DISCORD, 24.0),
            (marks::XBOX, 24.0),
            (sphere.as_str(), 64.0),
        ] {
            let bounds = parse(data).compute_tight_bounds().expect("a mark has bounds");
            assert!(bounds.left() >= -0.01 && bounds.top() >= -0.01, "{bounds:?}: {data}");
            assert!(bounds.right() <= side + 0.01 && bounds.bottom() <= side + 0.01, "{bounds:?}: {data}");
            assert!(bounds.width() > side / 3.0 && bounds.height() > side / 3.0, "{bounds:?}: {data}");
        }
    }

    /// The Bundle's D-pad, its corners arcs: it closes where it began and keeps to the plus's box.
    #[test]
    fn the_d_pads_arcs_keep_to_its_box() {
        let path = parse(
            "M 155.2,127.3 L 169.2,127.3 A 3,3 0 0 1 172.2,130.3 L 172.2,147.6 A 2,2 0 0 0 174.2,149.6 L 191.5,149.6 \
             A 3,3 0 0 1 194.5,152.6 L 194.5,166.6 A 3,3 0 0 1 191.5,169.6 Z",
        );
        let bounds = path.bounds();
        assert!((bounds.left() - 155.2).abs() < 1e-3 && (bounds.top() - 127.3).abs() < 1e-3);
        assert!((bounds.right() - 194.5).abs() < 1e-3 && (bounds.bottom() - 169.6).abs() < 1e-3);
    }
}
