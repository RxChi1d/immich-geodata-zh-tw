//! Wikidata 譯名消費層的共用防禦工具。
//!
//! 各國 handler 在消費 Wikidata 翻譯時共用的字串守門邏輯。緣起為印尼
//! handler 開發時實測踩到的三類髒資料（消歧括號直通、純拉丁 stale cache
//! 殘留、中英夾雜半翻譯），同類問題對 KR/TH 等翻譯型國家同樣可能發生，
//! 故集中於此單一實作，避免各 handler 平行維護而漂移。
//!
//! 注意各國「合法輸出」的差異：KR/ID 期望純中文（用
//! `is_valid_chinese_translation`）；TH 的官方英文為設計內的回退輸出，
//! 只能拒絕中英夾雜（用 `is_mixed_script`），不可要求純中文。

/// 移除譯名尾端的消歧括號（全形或半形）。
///
/// Reason: Wikidata 同名實體的 label 可能帶消歧後綴（KR 真實案例：光州
/// 廣域市的「北區 (光州廣域市)」；ID 真實案例：巴布亞省的「薩米縣
/// (巴布亞省)」），直接顯示會洩漏 Wikidata 內部消歧格式。
pub(super) fn strip_trailing_parenthetical(value: &str) -> String {
    let trimmed = value.trim();
    let stripped = if let Some(rest) = trimmed.strip_suffix('）') {
        rest.rfind('（').map(|start| &rest[..start])
    } else if let Some(rest) = trimmed.strip_suffix(')') {
        rest.rfind('(').map(|start| &rest[..start])
    } else {
        None
    };
    let Some(stripped) = stripped.map(str::trim_end) else {
        return trimmed.to_string();
    };
    // Reason: 本函式只認「尾端閉括號 + 最後一個同型左括號」，對巢狀或不成對
    //         的括號會切出壞字串——`甲（乙（丙））` 會變成未閉合的 `甲（乙`，
    //         `（甲）` 會整串被剝成空字串（行政區名變空，屬資料損壞）。
    //         因此只在結果既非空、也不殘留任何括號時才採用，否則原樣保留：
    //         寧可留著看得見的括號，也不要輸出壞掉或空的地名。
    if stripped.is_empty() || stripped.contains(['(', ')', '（', '）']) {
        return trimmed.to_string();
    }
    stripped.to_string()
}

/// 判斷字串是否含 CJK 漢字。
fn has_cjk(name: &str) -> bool {
    name.chars()
        .any(|ch| ('\u{4E00}'..='\u{9FFF}').contains(&ch))
}

/// 判斷譯名是否為「有效的中文翻譯」。
///
/// 有效條件：含 CJK 漢字、且不夾雜 ASCII 字母。
///
/// Reason: 期望純中文輸出的國家（KR/ID），任何非中文形態都應回退原文：
/// 1. 純拉丁字串（ID 真實案例「East Barito」）——英文 label 或 stale cache
///    殘留的舊回退鏈結果，不是中文翻譯。
/// 2. 中英夾雜（ID 真實案例：Kutai Barat 的 zh label「西Kutai區」）——
///    Wikidata 半翻譯髒資料。
///
/// 此判定一次涵蓋上述所有非中文形態，對來源（即時查詢、cache、stub）
/// 一視同仁，杜絕 cache 殘留繞過上游回退鏈修正的問題。
pub(super) fn is_valid_chinese_translation(name: &str) -> bool {
    has_cjk(name) && !name.chars().any(|ch| ch.is_ascii_alphabetic())
}

/// 判斷譯名是否為「中英夾雜」的損壞形態（同時含 CJK 漢字與 ASCII 字母）。
///
/// Reason: 設計上允許官方英文回退的國家（TH），純英文是合法輸出，
/// 不能套用 `is_valid_chinese_translation`；但中英夾雜（如「西Kutai區」
/// 型態的半翻譯）在任何國家都是髒資料，應回退原文。
pub(super) fn is_mixed_script(name: &str) -> bool {
    has_cjk(name) && name.chars().any(|ch| ch.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_trailing_parenthetical_rules() {
        // ID 真實案例：Wikidata 消歧後綴（半形）。
        assert_eq!(strip_trailing_parenthetical("薩米縣 (巴布亞省)"), "薩米縣");
        // KR 真實案例：光州廣域市轄區的消歧後綴。
        assert_eq!(strip_trailing_parenthetical("北區 (光州廣域市)"), "北區");
        // 全形括號同樣處理。
        assert_eq!(strip_trailing_parenthetical("薩米縣（巴布亞省）"), "薩米縣");
        // 無括號或括號不在尾端者不動。
        assert_eq!(strip_trailing_parenthetical("萬隆縣"), "萬隆縣");
        // 只有右括號、或左括號不在尾端，都不視為消歧後綴。
        assert_eq!(strip_trailing_parenthetical("甲）"), "甲）");
        assert_eq!(strip_trailing_parenthetical("甲（"), "甲（");
        // 巢狀括號會切出未閉合字串，須整串保留而非輸出壞值。
        assert_eq!(
            strip_trailing_parenthetical("甲（乙（丙））"),
            "甲（乙（丙））"
        );
        assert_eq!(strip_trailing_parenthetical("甲((乙)"), "甲((乙)");
        // 整串都被括號包住時剝除後為空，必須保留原值。
        assert_eq!(strip_trailing_parenthetical("（甲）"), "（甲）");
        assert_eq!(strip_trailing_parenthetical("()"), "()");
        assert_eq!(
            strip_trailing_parenthetical("邦加-勿里洞省"),
            "邦加-勿里洞省"
        );
    }

    #[test]
    fn valid_chinese_translation_detection() {
        // 有效中文翻譯：純中文（含連字號等非字母符號）。
        assert!(is_valid_chinese_translation("萬隆縣"));
        assert!(is_valid_chinese_translation("邦加-勿里洞省"));
        // 無效：純拉丁（英文 label 或 stale cache 殘留，ID 真實案例 East Barito）。
        assert!(!is_valid_chinese_translation("East Barito"));
        assert!(!is_valid_chinese_translation("Barito Timur"));
        // 無效：中英夾雜（ID 真實案例：Kutai Barat 的 zh label「西Kutai區」）。
        assert!(!is_valid_chinese_translation("西Kutai區"));
        // 無效：空字串。
        assert!(!is_valid_chinese_translation(""));
    }

    #[test]
    fn mixed_script_detection() {
        // 中英夾雜為損壞形態。
        assert!(is_mixed_script("西Kutai區"));
        // 純英文不算夾雜（TH 官方英文回退為合法輸出）。
        assert!(!is_mixed_script("Ban Dung"));
        // 純中文不算夾雜。
        assert!(!is_mixed_script("曼谷"));
        assert!(!is_mixed_script(""));
    }
}
