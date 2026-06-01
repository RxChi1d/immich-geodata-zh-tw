use std::collections::HashMap;
use std::path::PathBuf;

pub(super) const WGS84_EPSG: i32 = 4326;
pub(super) const TAIWAN_TWD97_EPSG: i32 = 3826;
pub(super) const JAPAN_ALBERS_PROJ4: &str = "+proj=aea +lat_1=30 +lat_2=45 +lat_0=37.5 +lon_0=138 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs";
pub(super) const KOREA_ALBERS_PROJ4: &str = "+proj=aea +lat_1=33 +lat_2=43 +lat_0=37 +lon_0=127.5 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs";

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
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct KoreaTranslations {
    pub(super) admin1_by_name: HashMap<String, String>,
    pub(super) admin2_by_parent: HashMap<(String, String), String>,
    pub(super) fallback_by_name: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ExtractContext {
    pub(super) korea_translations: KoreaTranslations,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CentroidPipeline {
    ProjectedEpsg(i32),
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
}

impl Country {
    pub(super) fn parse(code: &str) -> Result<Self, String> {
        match code {
            "TW" => Ok(Self::Taiwan),
            "JP" => Ok(Self::Japan),
            "KR" => Ok(Self::Korea),
            other => Err(format!("extract 尚未支援國家：{other}")),
        }
    }

    pub(super) fn code(self) -> &'static str {
        match self {
            Self::Taiwan => "TW",
            Self::Japan => "JP",
            Self::Korea => "KR",
        }
    }

    pub(super) fn centroid_pipeline(self) -> CentroidPipeline {
        match self {
            Self::Taiwan => CentroidPipeline::ProjectedEpsg(TAIWAN_TWD97_EPSG),
            Self::Japan => CentroidPipeline::DynamicUtm(JAPAN_ALBERS_PROJ4),
            Self::Korea => CentroidPipeline::DynamicUtm(KOREA_ALBERS_PROJ4),
        }
    }

    pub(super) fn extract_attribute_keys(self) -> &'static [&'static str] {
        match self {
            Self::Taiwan => &["COUNTYNAME", "TOWNNAME", "VILLNAME"],
            Self::Japan => &["N03_001", "N03_003", "N03_004", "N03_005"],
            Self::Korea => &["sidonm", "sggnm", "adm_nm"],
        }
    }
}

pub(super) fn korea_stub_source(source_path: &std::path::Path) -> Option<PathBuf> {
    source_path
        .parent()
        .map(|parent| parent.join("KR_wikidata_stub.json"))
        .filter(|path| path.exists())
}

pub(super) fn korea_translation_cache_path() -> PathBuf {
    std::path::Path::new("geoname_data").join("KR_wikidata_cache.json")
}
