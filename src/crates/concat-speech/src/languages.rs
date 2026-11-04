// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Spoken-language identifiers and normalization shared by the UI and Whisper.

/// Every language whisper's multilingual models know, by the code whisper
/// takes and the name the language calls itself - the way Settings lists
/// the interface's languages, so a person finds their own without reading
/// someone else's, and no list of a hundred names waits on 26
/// translations. In the order a reader of each script expects: the Latin
/// names alphabetically with their accents set aside, then each other
/// script together, alphabetically within it.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("af", "Afrikaans"),
    ("az", "Azərbaycanca"),
    ("id", "Bahasa Indonesia"),
    ("ms", "Bahasa Melayu"),
    ("jw", "Basa Jawa"),
    ("su", "Basa Sunda"),
    ("bs", "Bosanski"),
    ("br", "Brezhoneg"),
    ("ca", "Català"),
    ("cs", "Čeština"),
    ("sn", "ChiShona"),
    ("cy", "Cymraeg"),
    ("da", "Dansk"),
    ("de", "Deutsch"),
    ("et", "Eesti"),
    ("en", "English"),
    ("es", "Español"),
    ("eu", "Euskara"),
    ("fo", "Føroyskt"),
    ("fr", "Français"),
    ("gl", "Galego"),
    ("ha", "Hausa"),
    ("hr", "Hrvatski"),
    ("is", "Íslenska"),
    ("it", "Italiano"),
    ("sw", "Kiswahili"),
    ("ht", "Kreyòl ayisyen"),
    ("la", "Latina"),
    ("lv", "Latviešu"),
    ("lb", "Lëtzebuergesch"),
    ("lt", "Lietuvių"),
    ("ln", "Lingála"),
    ("hu", "Magyar"),
    ("mg", "Malagasy"),
    ("mt", "Malti"),
    ("mi", "Māori"),
    ("nl", "Nederlands"),
    ("no", "Norsk bokmål"),
    ("nn", "Norsk nynorsk"),
    ("oc", "Occitan"),
    ("haw", "ʻŌlelo Hawaiʻi"),
    ("uz", "Oʻzbekcha"),
    ("pl", "Polski"),
    ("pt", "Português"),
    ("ro", "Română"),
    ("sq", "Shqip"),
    ("sk", "Slovenčina"),
    ("sl", "Slovenščina"),
    ("so", "Soomaali"),
    ("fi", "Suomi"),
    ("sv", "Svenska"),
    ("tl", "Tagalog"),
    ("vi", "Tiếng Việt"),
    ("tr", "Türkçe"),
    ("tk", "Türkmençe"),
    ("yo", "Yorùbá"),
    ("el", "Ελληνικά"),
    ("ba", "Башҡортса"),
    ("be", "Беларуская"),
    ("bg", "Български"),
    ("kk", "Қазақ тілі"),
    ("mk", "Македонски"),
    ("mn", "Монгол"),
    ("ru", "Русский"),
    ("sr", "Српски"),
    ("tt", "Татарча"),
    ("tg", "Тоҷикӣ"),
    ("uk", "Українська"),
    ("hy", "Հայերեն"),
    ("yi", "ייִדיש"),
    ("he", "עברית"),
    ("ur", "اردو"),
    ("ar", "العربية"),
    ("sd", "سنڌي"),
    ("fa", "فارسی"),
    ("ps", "پښتو"),
    ("ne", "नेपाली"),
    ("mr", "मराठी"),
    ("sa", "संस्कृतम्"),
    ("hi", "हिन्दी"),
    ("as", "অসমীয়া"),
    ("bn", "বাংলা"),
    ("pa", "ਪੰਜਾਬੀ"),
    ("gu", "ગુજરાતી"),
    ("ta", "தமிழ்"),
    ("te", "తెలుగు"),
    ("kn", "ಕನ್ನಡ"),
    ("ml", "മലയാളം"),
    ("si", "සිංහල"),
    ("th", "ไทย"),
    ("lo", "ລາວ"),
    ("bo", "བོད་སྐད"),
    ("my", "မြန်မာ"),
    ("ka", "ქართული"),
    ("ko", "한국어"),
    ("am", "አማርኛ"),
    ("km", "ខ្មែរ"),
    ("zh", "中文"),
    ("ja", "日本語"),
    ("yue", "粵語"),
];

/// Canonical Whisper code; regional tags use their base language.
pub fn canonical(code: &str) -> Option<&'static str> {
    let base = code.trim().split(['-', '_']).next()?;
    LANGUAGES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(base))
        .map(|(known, _)| *known)
}

/// English-only models must never run multilingual detection.
pub fn requested(code: Option<&str>, english_only: bool) -> &'static str {
    if english_only {
        "en"
    } else {
        code.and_then(canonical).unwrap_or("auto")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_languages_auto_and_english_only_models_are_distinct() {
        assert_eq!(requested(None, false), "auto");
        assert_eq!(requested(Some("id"), false), "id");
        assert_eq!(requested(Some("ms"), false), "ms");
        assert_eq!(requested(Some("pt-BR"), false), "pt");
        assert_eq!(requested(Some(" PT_br "), false), "pt");
        assert_eq!(requested(Some("invalid"), false), "auto");
        assert_eq!(requested(Some("id"), true), "en");
        assert_eq!(requested(None, true), "en");
        let mut seen = std::collections::HashSet::new();
        for (code, name) in LANGUAGES {
            assert!(seen.insert(code), "duplicate {code}");
            assert!(!name.is_empty());
            assert_eq!(canonical(code), Some(*code));
        }
    }
}
