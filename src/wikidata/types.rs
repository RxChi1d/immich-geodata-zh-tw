use std::collections::{HashMap, HashSet};

pub const METADATA_OFFICIAL_EN: &str = "official_en";
pub const METADATA_OFFICIAL_TH: &str = "official_th";
/// 官方來源原文（language-agnostic）。
///
/// Reason: 部分國家來源（如印尼 BIG）無官方英文欄位，且搜尋字串為人工加上
/// 消歧前綴（`Kabupaten `）的字串；翻譯失敗回退時必須使用乾淨的官方原文，
/// 而非帶前綴的搜尋字串或被覆蓋的 Wikidata 英文 label。此 key 優先於
/// `official_en` / `official_th` 作為回退來源。
pub const METADATA_OFFICIAL_ORIGINAL: &str = "official_original";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminLevel {
    Admin1,
    Admin2,
}

impl AdminLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin1 => "admin_1",
            Self::Admin2 => "admin_2",
        }
    }
}

pub fn build_translation_item_id(
    level: AdminLevel,
    parent_chain: &[String],
    original_name: &str,
) -> String {
    let mut parts = vec![level.as_str().to_string()];
    parts.extend(parent_chain.iter().filter(|part| !part.is_empty()).cloned());
    if !original_name.is_empty() {
        parts.push(original_name.to_string());
    }
    parts.join("/")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationItem {
    pub id: String,
    pub level: AdminLevel,
    pub original_name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub parent_chain: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl TranslationItem {
    pub fn from_values(
        level: AdminLevel,
        original_name: impl AsRef<str>,
        source_lang: impl Into<String>,
        target_lang: impl Into<String>,
        parent_chain: Vec<String>,
        metadata: HashMap<String, String>,
    ) -> Result<Self, String> {
        let original_name = normalize_text(original_name.as_ref());
        if original_name.is_empty() {
            return Err("original_name 不可為空字串".to_string());
        }
        let parent_chain = parent_chain
            .into_iter()
            .map(|value| normalize_text(&value))
            .collect::<Vec<_>>();
        if parent_chain.first().is_none_or(|value| value.is_empty()) {
            return Err("parent_chain 至少需要包含國家碼".to_string());
        }
        let id = build_translation_item_id(level, &parent_chain, &original_name);
        Ok(Self {
            id,
            level,
            original_name,
            source_lang: source_lang.into(),
            target_lang: target_lang.into(),
            parent_chain,
            metadata,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationResult {
    pub translated: String,
    pub qid: Option<String>,
    pub source: String,
    pub used_lang: String,
    pub parent_verified: bool,
}

impl TranslationResult {
    pub fn original(name: &str) -> Self {
        Self {
            translated: name.to_string(),
            qid: None,
            source: "original".to_string(),
            used_lang: "original".to_string(),
            parent_verified: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TranslationDataset {
    items: Vec<TranslationItem>,
    pub level: AdminLevel,
    /// 國家的 Wikidata QID。
    ///
    /// Reason: P131 parent 驗證是 translator 的標準規則——admin2 對
    /// admin1 驗證、admin1（或缺少明確 parent 的 item）至少對國家
    /// 驗證。country_qid 為必填，確保未來新增國家時無法略過驗證。
    pub country_qid: String,
    pub source_lang: String,
    pub target_lang: String,
    pub deduplicated: bool,
}

impl TranslationDataset {
    pub fn new(
        items: Vec<TranslationItem>,
        level: AdminLevel,
        country_qid: impl Into<String>,
        source_lang: impl Into<String>,
        target_lang: impl Into<String>,
        deduplicated: bool,
    ) -> Result<Self, String> {
        if items.iter().any(|item| item.level != level) {
            return Err("所有 TranslationItem 必須擁有相同的 level".to_string());
        }
        let country_qid = country_qid.into();
        if country_qid.is_empty() {
            return Err("country_qid 不可為空（P131 parent 驗證為標準規則）".to_string());
        }
        Ok(Self {
            items,
            level,
            country_qid,
            source_lang: source_lang.into(),
            target_lang: target_lang.into(),
            deduplicated,
        })
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[TranslationItem] {
        &self.items
    }
}

#[derive(Clone, Debug)]
pub struct TranslationDatasetBuilder {
    country_code: String,
    country_qid: String,
    source_lang: String,
    target_lang: String,
}

impl TranslationDatasetBuilder {
    pub fn new(
        country_code: impl AsRef<str>,
        country_qid: impl Into<String>,
        source_lang: impl Into<String>,
        target_lang: impl Into<String>,
    ) -> Result<Self, String> {
        let country_code = normalize_text(country_code.as_ref());
        if country_code.is_empty() {
            return Err("country_code 不可為空".to_string());
        }
        let country_qid = country_qid.into();
        if country_qid.is_empty() {
            return Err("country_qid 不可為空（P131 parent 驗證為標準規則）".to_string());
        }
        Ok(Self {
            country_code,
            country_qid,
            source_lang: source_lang.into(),
            target_lang: target_lang.into(),
        })
    }

    pub fn build_admin1_names<S>(
        &self,
        names: impl IntoIterator<Item = S>,
    ) -> Result<TranslationDataset, String>
    where
        S: AsRef<str>,
    {
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for name in names {
            let name = normalize_text(name.as_ref());
            if name.is_empty() || !seen.insert(name.clone()) {
                continue;
            }
            items.push(TranslationItem::from_values(
                AdminLevel::Admin1,
                name,
                self.source_lang.clone(),
                self.target_lang.clone(),
                vec![self.country_code.clone()],
                HashMap::new(),
            )?);
        }
        TranslationDataset::new(
            items,
            AdminLevel::Admin1,
            self.country_qid.clone(),
            self.source_lang.clone(),
            self.target_lang.clone(),
            true,
        )
    }

    pub fn build_admin2_pairs<P, N>(
        &self,
        pairs: impl IntoIterator<Item = (P, N)>,
        deduplicate: bool,
    ) -> Result<TranslationDataset, String>
    where
        P: AsRef<str>,
        N: AsRef<str>,
    {
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for (parent, name) in pairs {
            let parent = normalize_text(parent.as_ref());
            let name = normalize_text(name.as_ref());
            if parent.is_empty() || name.is_empty() {
                continue;
            }
            let dedupe_key = (parent.clone(), name.clone());
            if deduplicate && !seen.insert(dedupe_key) {
                continue;
            }
            items.push(TranslationItem::from_values(
                AdminLevel::Admin2,
                name,
                self.source_lang.clone(),
                self.target_lang.clone(),
                vec![self.country_code.clone(), parent],
                HashMap::new(),
            )?);
        }
        TranslationDataset::new(
            items,
            AdminLevel::Admin2,
            self.country_qid.clone(),
            self.source_lang.clone(),
            self.target_lang.clone(),
            deduplicate,
        )
    }
}

fn normalize_text(value: &str) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_id_includes_level_parent_chain_and_name() {
        let item = TranslationItem::from_values(
            AdminLevel::Admin2,
            "중구",
            "ko",
            "zh-tw",
            vec!["KR".to_string(), "서울특별시".to_string()],
            HashMap::new(),
        )
        .unwrap();

        assert_eq!(item.id, "admin_2/KR/서울특별시/중구");
    }

    #[test]
    fn builder_deduplicates_admin2_by_parent_context() {
        let builder = TranslationDatasetBuilder::new("KR", "Q884", "ko", "zh-tw").unwrap();
        let dataset = builder
            .build_admin2_pairs(
                [
                    ("서울특별시".to_string(), "중구".to_string()),
                    ("부산광역시".to_string(), "중구".to_string()),
                    ("서울특별시".to_string(), "중구".to_string()),
                ],
                true,
            )
            .unwrap();

        assert_eq!(dataset.len(), 2);
        assert_eq!(dataset.items()[0].id, "admin_2/KR/서울특별시/중구");
        assert_eq!(dataset.items()[1].id, "admin_2/KR/부산광역시/중구");
    }
}
