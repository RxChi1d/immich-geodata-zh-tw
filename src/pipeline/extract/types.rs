use std::collections::HashMap;
use std::path::PathBuf;

pub(super) const WGS84_EPSG: i32 = 4326;
pub(super) const TAIWAN_TWD97_EPSG: i32 = 3826;
pub(super) const JAPAN_ALBERS_PROJ4: &str = "+proj=aea +lat_1=30 +lat_2=45 +lat_0=37.5 +lon_0=138 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs";
pub(super) const KOREA_ALBERS_PROJ4: &str = "+proj=aea +lat_1=33 +lat_2=43 +lat_0=37 +lon_0=127.5 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs";
pub(super) const THAILAND_ALBERS_PROJ4: &str = "+proj=aea +lat_1=5 +lat_2=21 +lat_0=13 +lon_0=101 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs";
// 印尼正式定案的單一 Albers 等積投影（涵蓋全境東經 95°~141°）。
// standard parallels 置於南北邊界內側、central meridian 取群島經度中心 118°；
// 橢球採 GRS80（與 SRGI 2013 / WGS84 一致）。
//
// Reason: 階段二實驗以全精度 BIG desa 圖資比對 Albers 單一投影 vs dynamic UTM，
// centroid 差異中位 0.011 m、最大 32.98 m（皆遠小於村級行政區粒度），故採單一
// Albers，pipeline 不需逐 polygon 切換 UTM 帶、行為單純可重現、與 TH 先例一致。
// 詳見 docs/research/idn-handler-projection-coordinate-experiment.md。
pub(super) const INDONESIA_ALBERS_PROJ4: &str = "+proj=aea +lat_1=1 +lat_2=-8 +lat_0=-3 +lon_0=118 +x_0=0 +y_0=0 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs";

pub(super) type Coordinate = (f64, f64);
pub(super) type LinearRing = Vec<Coordinate>;
pub(super) type PolygonRings = Vec<LinearRing>;
pub(super) type MultiPolygonRings = Vec<PolygonRings>;

#[derive(Clone, Debug)]
pub(super) struct Feature {
    pub(super) geometry: FeatureGeometry,
    pub(super) attributes: FeatureAttributes,
    pub(super) crs: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ExtractRow {
    pub(super) latitude: String,
    pub(super) longitude: String,
    pub(super) latitude_key: f64,
    pub(super) longitude_key: f64,
    pub(super) country: String,
    pub(super) admin_1: String,
    pub(super) admin_2: String,
    pub(super) admin_3: String,
    pub(super) admin_4: String,
}

impl ExtractRow {
    pub(super) fn from_point(
        latitude: f64,
        longitude: f64,
        country: impl Into<String>,
        admin_1: impl Into<String>,
        admin_2: impl Into<String>,
        admin_3: impl Into<String>,
        admin_4: impl Into<String>,
    ) -> Self {
        Self {
            latitude: latitude.to_string(),
            longitude: longitude.to_string(),
            latitude_key: latitude,
            longitude_key: longitude,
            country: country.into(),
            admin_1: admin_1.into(),
            admin_2: admin_2.into(),
            admin_3: admin_3.into(),
            admin_4: admin_4.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum FeatureAttributes {
    Taiwan {
        countyname: Option<String>,
        townname: Option<String>,
        villname: Option<String>,
    },
    Japan {
        n03_001: Option<String>,
        n03_003: Option<String>,
        n03_004: Option<String>,
        n03_005: Option<String>,
    },
    Korea {
        sidonm: Option<String>,
        sggnm: Option<String>,
        adm_nm: Option<String>,
    },
    Thailand {
        adm1_name: Option<String>,
        adm1_name1: Option<String>,
        adm2_name: Option<String>,
        adm2_name1: Option<String>,
        adm3_name: Option<String>,
        adm3_name1: Option<String>,
    },
    Indonesia {
        // BIG desa（村級）圖資的行政區欄位：
        // WADMPR=省、WADMKK=縣市、WADMKC=郡、WADMKD=村。
        wadmpr: Option<String>,
        wadmkk: Option<String>,
        wadmkc: Option<String>,
        wadmkd: Option<String>,
    },
}

impl FeatureAttributes {
    pub(super) fn empty(country: Country) -> Self {
        match country {
            Country::Taiwan => Self::Taiwan {
                countyname: None,
                townname: None,
                villname: None,
            },
            Country::Japan => Self::Japan {
                n03_001: None,
                n03_003: None,
                n03_004: None,
                n03_005: None,
            },
            Country::Korea => Self::Korea {
                sidonm: None,
                sggnm: None,
                adm_nm: None,
            },
            Country::Thailand => Self::Thailand {
                adm1_name: None,
                adm1_name1: None,
                adm2_name: None,
                adm2_name1: None,
                adm3_name: None,
                adm3_name1: None,
            },
            Country::Indonesia => Self::Indonesia {
                wadmpr: None,
                wadmkk: None,
                wadmkc: None,
                wadmkd: None,
            },
        }
    }

    pub(super) fn set(&mut self, key: &str, value: String) {
        match self {
            Self::Taiwan {
                countyname,
                townname,
                villname,
            } => match key {
                "COUNTYNAME" => *countyname = Some(value),
                "TOWNNAME" => *townname = Some(value),
                "VILLNAME" => *villname = Some(value),
                _ => {}
            },
            Self::Japan {
                n03_001,
                n03_003,
                n03_004,
                n03_005,
            } => match key {
                "N03_001" => *n03_001 = Some(value),
                "N03_003" => *n03_003 = Some(value),
                "N03_004" => *n03_004 = Some(value),
                "N03_005" => *n03_005 = Some(value),
                _ => {}
            },
            Self::Korea {
                sidonm,
                sggnm,
                adm_nm,
            } => match key {
                "sidonm" => *sidonm = Some(value),
                "sggnm" => *sggnm = Some(value),
                "adm_nm" => *adm_nm = Some(value),
                _ => {}
            },
            Self::Thailand {
                adm1_name,
                adm1_name1,
                adm2_name,
                adm2_name1,
                adm3_name,
                adm3_name1,
            } => match key {
                "adm1_name" => *adm1_name = Some(value),
                "adm1_name1" => *adm1_name1 = Some(value),
                "adm2_name" => *adm2_name = Some(value),
                "adm2_name1" => *adm2_name1 = Some(value),
                "adm3_name" => *adm3_name = Some(value),
                "adm3_name1" => *adm3_name1 = Some(value),
                _ => {}
            },
            Self::Indonesia {
                wadmpr,
                wadmkk,
                wadmkc,
                wadmkd,
            } => match key {
                "WADMPR" => *wadmpr = Some(value),
                "WADMKK" => *wadmkk = Some(value),
                "WADMKC" => *wadmkc = Some(value),
                "WADMKD" => *wadmkd = Some(value),
                _ => {}
            },
        }
    }

    pub(super) fn get(&self, key: &str) -> Option<&str> {
        match self {
            Self::Taiwan {
                countyname,
                townname,
                villname,
            } => match key {
                "COUNTYNAME" => countyname.as_deref(),
                "TOWNNAME" => townname.as_deref(),
                "VILLNAME" => villname.as_deref(),
                _ => None,
            },
            Self::Japan {
                n03_001,
                n03_003,
                n03_004,
                n03_005,
            } => match key {
                "N03_001" => n03_001.as_deref(),
                "N03_003" => n03_003.as_deref(),
                "N03_004" => n03_004.as_deref(),
                "N03_005" => n03_005.as_deref(),
                _ => None,
            },
            Self::Korea {
                sidonm,
                sggnm,
                adm_nm,
            } => match key {
                "sidonm" => sidonm.as_deref(),
                "sggnm" => sggnm.as_deref(),
                "adm_nm" => adm_nm.as_deref(),
                _ => None,
            },
            Self::Thailand {
                adm1_name,
                adm1_name1,
                adm2_name,
                adm2_name1,
                adm3_name,
                adm3_name1,
            } => match key {
                "adm1_name" => adm1_name.as_deref(),
                "adm1_name1" => adm1_name1.as_deref(),
                "adm2_name" => adm2_name.as_deref(),
                "adm2_name1" => adm2_name1.as_deref(),
                "adm3_name" => adm3_name.as_deref(),
                "adm3_name1" => adm3_name1.as_deref(),
                _ => None,
            },
            Self::Indonesia {
                wadmpr,
                wadmkk,
                wadmkc,
                wadmkd,
            } => match key {
                "WADMPR" => wadmpr.as_deref(),
                "WADMKK" => wadmkk.as_deref(),
                "WADMKC" => wadmkc.as_deref(),
                "WADMKD" => wadmkd.as_deref(),
                _ => None,
            },
        }
    }
}

/// 各國共用的 Wikidata 翻譯查詢表。
///
/// `fallback_by_name` 僅保留「全國無歧義」的名稱（同名不同譯的項目會被
/// 剔除），避免跨上層行政區的同名單位拿到錯誤翻譯。
#[derive(Clone, Debug, Default)]
pub(super) struct WikidataTranslations {
    pub(super) admin1_by_name: HashMap<String, String>,
    pub(super) admin2_by_parent: HashMap<String, HashMap<String, String>>,
    pub(super) fallback_by_name: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ExtractContext {
    pub(super) korea_translations: WikidataTranslations,
    pub(super) thailand_translations: WikidataTranslations,
    pub(super) indonesia_translations: WikidataTranslations,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CentroidPipeline {
    ProjectedEpsg(i32),
    ProjectedProj4(&'static str),
    DynamicUtm(&'static str),
}

#[derive(Clone, Debug)]
pub(super) enum FeatureGeometry {
    Point(Coordinate),
    Polygon(PolygonRings),
    MultiPolygon(MultiPolygonRings),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Country {
    Taiwan,
    Japan,
    Korea,
    Thailand,
    Indonesia,
}

impl Country {
    /// 所有擁有 extract handler 的國家（依代碼字母序）。
    ///
    /// Reason: 這是「哪些國家有 handler」的單一事實來源；CLI 的
    /// handler 國家清單由此導出，新增國家時不需（也不能）另行同步。
    pub(super) const ALL: [Country; 5] = [
        Country::Indonesia,
        Country::Japan,
        Country::Korea,
        Country::Thailand,
        Country::Taiwan,
    ];

    pub(super) fn parse(code: &str) -> Result<Self, String> {
        match code {
            "TW" => Ok(Self::Taiwan),
            "JP" => Ok(Self::Japan),
            "KR" => Ok(Self::Korea),
            "TH" => Ok(Self::Thailand),
            "ID" => Ok(Self::Indonesia),
            other => Err(format!("extract 尚未支援國家：{other}")),
        }
    }

    pub(super) fn code(self) -> &'static str {
        match self {
            Self::Taiwan => "TW",
            Self::Japan => "JP",
            Self::Korea => "KR",
            Self::Thailand => "TH",
            Self::Indonesia => "ID",
        }
    }

    pub(super) fn centroid_pipeline(self) -> CentroidPipeline {
        match self {
            Self::Taiwan => CentroidPipeline::ProjectedEpsg(TAIWAN_TWD97_EPSG),
            Self::Japan => CentroidPipeline::DynamicUtm(JAPAN_ALBERS_PROJ4),
            Self::Korea => CentroidPipeline::DynamicUtm(KOREA_ALBERS_PROJ4),
            // Reason: COD-AB TH 的官方 center_lat/center_lon 是代表點，
            //         但 Immich 以最近單點查詢行政區；實測 polygon centroid
            //         命中率較高，且 dynamic UTM 對泰國沒有實質改善。
            Self::Thailand => CentroidPipeline::ProjectedProj4(THAILAND_ALBERS_PROJ4),
            // Reason: 比照 TH 走 static Albers 路徑（非 dynamic UTM）。階段二實驗
            //         確認 Albers vs dynamic UTM centroid 差異公尺級以下，無實質收益，
            //         參數見 INDONESIA_ALBERS_PROJ4。
            Self::Indonesia => CentroidPipeline::ProjectedProj4(INDONESIA_ALBERS_PROJ4),
        }
    }

    /// 是否在 centroid 計算前把 MultiPolygon 拆成每 part 一筆 feature。
    ///
    /// Reason: CLAUDE.md 與 TH 文件記載「multipart polygon 每個部分各出一列」
    /// 為預期行為——同一行政區若由多個不相連的 polygon 組成，每個 polygon 各取
    /// 中心點輸出一列，避免群島合併 centroid 落海。印尼 BIG desa 圖資含大量散島
    /// multipart，故啟用；階段二命中率實驗（96.99%）即建立在「每 part 一列、共
    /// 104,470 候選點」之上（見 docs/research）。其餘國家維持既有 per-feature
    /// 合併 centroid 行為，避免回退既有輸出。
    pub(super) fn splits_multipolygon_parts(self) -> bool {
        matches!(self, Self::Indonesia)
    }

    pub(super) fn extract_attribute_keys(self) -> &'static [&'static str] {
        match self {
            Self::Taiwan => &["COUNTYNAME", "TOWNNAME", "VILLNAME"],
            Self::Japan => &["N03_001", "N03_003", "N03_004", "N03_005"],
            Self::Korea => &["sidonm", "sggnm", "adm_nm"],
            Self::Thailand => &[
                "adm1_name",
                "adm1_name1",
                "adm2_name",
                "adm2_name1",
                "adm3_name",
                "adm3_name1",
            ],
            Self::Indonesia => &["WADMPR", "WADMKK", "WADMKC", "WADMKD"],
        }
    }
}

/// 尋找與來源檔同目錄的 `{CC}_wikidata_stub.json`（fixture/離線測試用）。
pub(super) fn wikidata_stub_source(
    source_path: &std::path::Path,
    country_code: &str,
) -> Option<PathBuf> {
    source_path
        .parent()
        .map(|parent| parent.join(format!("{country_code}_wikidata_stub.json")))
        .filter(|path| path.exists())
}

/// 該國 Wikidata 翻譯快取的標準路徑。
pub(super) fn wikidata_cache_path(country_code: &str) -> PathBuf {
    std::path::Path::new("geoname_data").join(format!("{country_code}_wikidata_cache.json"))
}
