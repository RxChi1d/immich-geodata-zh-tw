use std::sync::OnceLock;

use regex::Regex;

static HAN_REGEX: OnceLock<Regex> = OnceLock::new();

pub fn is_han(character: char) -> bool {
    let mut buffer = [0_u8; 4];
    let value = character.encode_utf8(&mut buffer);
    han_regex().is_match(value)
}

pub fn includes_han(text: &str) -> bool {
    han_regex().is_match(text)
}

pub fn is_han_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| character == '-' || is_han(character))
}

fn han_regex() -> &'static Regex {
    HAN_REGEX.get_or_init(|| {
        Regex::new(r"\p{scx:Han}")
            .expect("Rust regex should support Unicode Script_Extensions=Han via scx")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_regex_script_extensions_han_smoke() {
        for character in ['漢', '𠀀', '㐀', '々', '〆', '〇', '〡', '·', '・'] {
            assert!(is_han(character), "{character}");
        }
        assert!(!is_han('A'));
    }
}
