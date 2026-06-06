//! 「明確簡體字」白名單的單一事實來源。
//!
//! 偵測（`translator::warn_if_simplified`）與修正（印尼 handler 的
//! `fix_simplified_chars`）共用同一份對照表，避免兩份平行清單漂移：
//! 若某字只加入其中一份，告警與修正行為會悄然分歧。

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
pub(crate) const SIMPLIFIED_TO_TRADITIONAL: &[(char, char)] = &[
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

/// 判斷字元是否為「明確簡體字」（簡體獨有、無正體歧義）。
///
/// Reason: 僅用於非破壞性告警偵測。刻意只列簡體獨有字，不含 `里`/`占`/`岩`
/// 等本身即合法正體的字，以避免對已正確的繁體 label 誤報。此清單可隨累積的
/// 各國案例擴充，但永遠不應加入有正體用途的字。
pub(crate) fn is_simplified_char(ch: char) -> bool {
    SIMPLIFIED_TO_TRADITIONAL
        .iter()
        .any(|(simplified, _)| *simplified == ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_left_column_is_simplified_only() {
        // 對照表左欄不得含「本身為合法正體」的字（右欄即其正體形）。
        for (simplified, traditional) in SIMPLIFIED_TO_TRADITIONAL {
            assert_ne!(
                simplified, traditional,
                "簡體欄與正體欄不應相同：{simplified}"
            );
            assert!(is_simplified_char(*simplified));
            assert!(
                !is_simplified_char(*traditional),
                "正體字 {traditional} 不應被判為簡體"
            );
        }
    }
}
