//! 印尼 admin1/admin2 最終譯名的字形與字尾正規化。
//!
//! 兩段後處理在 handler 消費層（`indonesia.rs`）統一施作，使 live（Wikidata
//! 即時查詢）與 fixture（stub）兩條來源路徑得到一致的最終形態：
//! 1. `fix_simplified_chars`：安全字元級簡轉繁（白名單），只修簡體獨有字。
//! 2. `normalize_admin1_suffix`：admin1 補「省」字尾（特區／首都／已含字尾不動）。

/// 安全的簡轉繁字元對照（僅含「簡體獨有、無正體歧義」的字）。
///
/// Reason: OpenCC 的 s2t（STCharacters）對部分「本身即正體」的字會過度轉換
/// 成異體（如 `里→裏`、`占→佔`、`岩→巖`、`干→乾`、`布→佈`、`托→託`、
/// `群→羣`），把已正確的繁體專名改壞。實測對真實 ID 輸出套用完整 s2t 會
/// 改壞 24 個正確譯名、只修好 1 個（`亚→亞`）。因此改用「白名單」字元對照，
/// 只轉換簡體獨有、轉成正體零歧義的字，徹底避免過度轉換。
///
/// 清單以實際出現於 Wikidata 中文 label 的簡體字為基礎，並補入常見、無正體
/// 歧義的簡體字以涵蓋未來 label 漂移；不含任何「本身為合法正體」的字。
const SIMPLIFIED_TO_TRADITIONAL: &[(char, char)] = &[
    ('亚', '亞'),
    ('东', '東'),
    ('县', '縣'),
    ('区', '區'),
    ('岛', '島'),
    ('湾', '灣'),
    ('门', '門'),
    ('华', '華'),
    ('汉', '漢'),
    ('观', '觀'),
    ('旧', '舊'),
    ('属', '屬'),
    ('寿', '壽'),
    ('万', '萬'),
    ('丰', '豐'),
    ('严', '嚴'),
    ('丽', '麗'),
    ('举', '舉'),
    ('义', '義'),
    ('乐', '樂'),
    ('习', '習'),
    ('乡', '鄉'),
    ('书', '書'),
    ('买', '買'),
    ('争', '爭'),
    ('亲', '親'),
    ('们', '們'),
    ('单', '單'),
    ('卖', '賣'),
    ('国', '國'),
    ('图', '圖'),
    ('壮', '壯'),
    ('节', '節'),
    ('苏', '蘇'),
    ('蓝', '藍'),
];

/// 套用安全字元級簡轉繁。
///
/// Reason: 逐字以 `SIMPLIFIED_TO_TRADITIONAL` 白名單替換，未在表中的字一律
/// 原樣保留（含已正確的繁體字），確保對已正確的繁體專名為冪等、零回歸。
pub(super) fn fix_simplified_chars(name: &str) -> String {
    name.chars()
        .map(|ch| {
            SIMPLIFIED_TO_TRADITIONAL
                .iter()
                .find(|(simplified, _)| *simplified == ch)
                .map(|(_, traditional)| *traditional)
                .unwrap_or(ch)
        })
        .collect()
}

/// admin1 省名補「省」字尾正規化。
///
/// 規則：若譯名已以「省／特區／市」結尾則不動，否則補「省」。
///
/// Reason: 多數 Wikidata 省 label 已含「省」字尾，但少數（如「中加里曼丹」
/// 「哥倫打洛」「北蘇門答臘」）缺字尾。雅加達（首都特區）不含上述字尾且語意
/// 非「省」，明確列為例外，避免「雅加達省」。
pub(super) fn normalize_admin1_suffix(name: &str) -> String {
    const KEEP_SUFFIXES: &[&str] = &["省", "特區", "市"];
    // Reason: 首都特區「雅加達」不含一般字尾，但語意上非「省」，明確排除。
    const KEEP_AS_IS: &[&str] = &["雅加達"];
    let trimmed = name.trim();
    if KEEP_AS_IS.contains(&trimmed) || KEEP_SUFFIXES.iter().any(|suffix| trimmed.ends_with(suffix))
    {
        trimmed.to_string()
    } else {
        format!("{trimmed}省")
    }
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
/// Reason: 翻譯的目的是中文輸出，任何非中文形態都應回退 BIG 印尼文原文：
/// 1. 純拉丁字串（如「East Barito」）——英文 label 或 stale cache 殘留的
///    舊回退鏈結果，不是中文翻譯。
/// 2. 中英夾雜（如 Kutai Barat 的 zh label「西Kutai區」）——Wikidata
///    半翻譯髒資料。
///
/// 此判定一次涵蓋上述所有非中文形態，對來源（即時查詢、cache、stub）
/// 一視同仁，杜絕 cache 殘留繞過上游回退鏈修正的問題。
pub(super) fn is_valid_chinese_translation(name: &str) -> bool {
    has_cjk(name) && !name.chars().any(|ch| ch.is_ascii_alphabetic())
}

/// 移除譯名尾端的消歧括號（全形或半形）。
///
/// Reason: Wikidata 同名實體的 label 可能帶消歧後綴（真實案例：巴布亞省
/// 的「薩米縣 (巴布亞省)」），直接顯示會洩漏 Wikidata 內部消歧格式。
/// 比照 KR handler 的 `strip_trailing_parenthetical` 先例去除。
pub(super) fn strip_trailing_parenthetical(value: &str) -> String {
    let trimmed = value.trim();
    let stripped = if let Some(rest) = trimmed.strip_suffix('）') {
        rest.rfind('（').map(|start| &rest[..start])
    } else if let Some(rest) = trimmed.strip_suffix(')') {
        rest.rfind('(').map(|start| &rest[..start])
    } else {
        None
    };
    stripped.unwrap_or(trimmed).trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_trailing_parenthetical_rules() {
        // 真實案例：Wikidata 消歧後綴（半形）。
        assert_eq!(strip_trailing_parenthetical("薩米縣 (巴布亞省)"), "薩米縣");
        // 全形括號同樣處理。
        assert_eq!(strip_trailing_parenthetical("薩米縣（巴布亞省）"), "薩米縣");
        // 無括號或括號不在尾端者不動。
        assert_eq!(strip_trailing_parenthetical("萬隆縣"), "萬隆縣");
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
        // 無效：純拉丁（英文 label 或 stale cache 殘留，真實案例 East Barito）。
        assert!(!is_valid_chinese_translation("East Barito"));
        assert!(!is_valid_chinese_translation("Barito Timur"));
        // 無效：中英夾雜（真實案例：Kutai Barat 的 zh label「西Kutai區」）。
        assert!(!is_valid_chinese_translation("西Kutai區"));
        // 無效：空字串。
        assert!(!is_valid_chinese_translation(""));
    }

    #[test]
    fn fix_simplified_chars_converts_known_simplified() {
        // Q2：巴布亚省（簡體「亚」）→ 巴布亞省（繁體「亞」）。
        assert_eq!(fix_simplified_chars("巴布亚省"), "巴布亞省");
    }

    #[test]
    fn fix_simplified_chars_is_idempotent_on_correct_traditional() {
        // 已正確的繁體專名不被過度轉換（這是改用白名單而非完整 s2t 的關鍵）。
        // OpenCC s2t 會把以下字改壞：里→裏、占→佔、岩→巖、干→乾、群→羣。
        for name in [
            "巴釐省",
            "井里汶縣",
            "勿里洞縣",
            "峇里巴板",
            "占碑省",
            "岩望縣",
            "北干巴魯",
            "千島群島",
            "邦加-勿里洞省",
            "肯達里",
        ] {
            assert_eq!(fix_simplified_chars(name), name, "不應改壞正確繁體：{name}");
        }
    }

    #[test]
    fn normalize_admin1_suffix_rules() {
        // Q3：缺字尾者補「省」。
        assert_eq!(normalize_admin1_suffix("中加里曼丹"), "中加里曼丹省");
        assert_eq!(normalize_admin1_suffix("哥倫打洛"), "哥倫打洛省");
        assert_eq!(normalize_admin1_suffix("北蘇門答臘"), "北蘇門答臘省");
        // 已含省／特區字尾者不動，不疊字。
        assert_eq!(normalize_admin1_suffix("中爪哇省"), "中爪哇省");
        assert_eq!(normalize_admin1_suffix("亞齊特區"), "亞齊特區");
        assert_eq!(normalize_admin1_suffix("西南巴布亞省"), "西南巴布亞省");
        // 首都特區「雅加達」不變成「雅加達省」。
        assert_eq!(normalize_admin1_suffix("雅加達"), "雅加達");
    }

    #[test]
    fn admin1_pipeline_fix_then_suffix() {
        // 簡轉繁與補省串接：巴布亚省 → 巴布亞省（已含字尾，不補）。
        let after_fix = fix_simplified_chars("巴布亚省");
        assert_eq!(normalize_admin1_suffix(&after_fix), "巴布亞省");
        // 補省案例：北蘇門答臘 → 北蘇門答臘省。
        assert_eq!(
            normalize_admin1_suffix(&fix_simplified_chars("北蘇門答臘")),
            "北蘇門答臘省"
        );
    }
}
