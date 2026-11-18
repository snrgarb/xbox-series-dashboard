//! Windows' own region and language (#277): what the PC Game Pass catalogue is asked in until Settings names a
//! region. The region is the user's home location (`GetUserDefaultGeoName`, Windows 10 1709 on: an ISO 3166 code,
//! or a UN M.49 number where the place has none), else the country of the user's regional format
//! (`GetUserDefaultLocaleName`, `en-GB`); the language Windows' display language (`GetUserPreferredUILanguages`),
//! else the regional format's. Only these go to the catalogue: nothing else of the user.
//!
//! `DASHBOARD_WINDOWS_REGION` (`GB`, or `GB en-GB`) stands in for both, for tests and on a machine that is not
//! Windows.

/// What Windows says: the region's two-letter code, and a language tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Windows {
    pub region: Option<String>,
    pub language: Option<String>,
}

/// The environment variable that stands in for Windows' word.
pub const STAND_IN: &str = "DASHBOARD_WINDOWS_REGION";

/// Windows' region and language, or the stand-in's.
pub fn windows() -> Windows {
    if let Some(text) = std::env::var(STAND_IN).ok().filter(|text| !text.trim().is_empty()) {
        return stand_in(&text);
    }
    read()
}

/// `GB`, or `GB en-GB`.
fn stand_in(text: &str) -> Windows {
    let mut words = text.split_whitespace();
    let region = words.next().filter(|code| is_code(code)).map(str::to_ascii_uppercase);
    let language = words.next().map(str::to_string);
    let language = language.or_else(|| region.as_ref().map(|code| format!("en-{code}")));
    Windows { region, language }
}

fn is_code(text: &str) -> bool {
    text.len() == 2 && text.bytes().all(|b| b.is_ascii_alphabetic())
}

/// A locale name's country: `en-GB` is `GB`, `zh-Hant-TW` is `TW`.
pub fn country_of(locale: &str) -> Option<String> {
    locale.split(['-', '_']).skip(1).find(|part| is_code(part)).map(str::to_ascii_uppercase)
}

/// The region and language from what Windows answered: a geo name that is no two-letter code (a number) gives way to
/// the regional format's country; a display language to the regional format.
pub fn judged(geo: Option<&str>, ui_language: Option<&str>, locale: Option<&str>) -> Windows {
    let region = geo.filter(|geo| is_code(geo)).map(str::to_ascii_uppercase).or_else(|| locale.and_then(country_of));
    let tag = |text: &str| text.contains('-') && text.len() <= 20;
    let language = ui_language.filter(|t| tag(t)).or(locale.filter(|t| tag(t))).map(str::to_string);
    Windows { region, language }
}

#[cfg(windows)]
fn read() -> Windows {
    use windows_sys::Win32::Globalization::{
        GetUserDefaultGeoName, GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
    };
    let text = |buffer: &[u16]| {
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..end])).filter(|t| !t.is_empty())
    };
    let mut geo = [0u16; 16];
    // SAFETY: the buffer's length is given; Windows writes at most that many characters.
    let geo = (unsafe { GetUserDefaultGeoName(geo.as_mut_ptr(), geo.len() as i32) } > 0).then(|| text(&geo)).flatten();
    let mut locale = [0u16; 85];
    // SAFETY: as above; 85 is LOCALE_NAME_MAX_LENGTH.
    let locale = (unsafe { GetUserDefaultLocaleName(locale.as_mut_ptr(), locale.len() as i32) } > 0)
        .then(|| text(&locale))
        .flatten();
    let (mut count, mut length) = (0u32, 256u32);
    let mut languages = [0u16; 256];
    // SAFETY: as above: the list is double-null-terminated within `length` characters; its first name is taken.
    let ok = unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, languages.as_mut_ptr(), &mut length) };
    let ui = if ok != 0 && count > 0 { text(&languages) } else { None };
    judged(geo.as_deref(), ui.as_deref(), locale.as_deref())
}

#[cfg(not(windows))]
fn read() -> Windows {
    Windows { region: None, language: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_location_first_else_the_regional_formats_country() {
        let w = judged(Some("GB"), Some("en-US"), Some("en-GB"));
        assert_eq!((w.region.as_deref(), w.language.as_deref()), (Some("GB"), Some("en-US")));
        // A UN M.49 number: the regional format's country.
        let w = judged(Some("419"), None, Some("es-MX"));
        assert_eq!((w.region.as_deref(), w.language.as_deref()), (Some("MX"), Some("es-MX")));
        assert_eq!(judged(None, None, None), Windows { region: None, language: None });
        assert_eq!(country_of("zh-Hant-TW").as_deref(), Some("TW"));
        assert_eq!(stand_in("gb"), Windows { region: Some("GB".into()), language: Some("en-GB".into()) });
        assert_eq!(stand_in("DE de-DE").language.as_deref(), Some("de-DE"));
    }
}
