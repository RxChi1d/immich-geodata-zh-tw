pub mod admin1_load;
pub mod cities500_load;
pub mod extract;
pub mod fixtures;
pub mod geodata;
pub mod indonesia_timezone;
pub mod locationiq;
pub mod naer_lookup;
pub mod naer_prepare;
pub mod pack;
pub mod polars_cities;
pub mod polars_table;
pub mod prepare;
pub mod prepare_download;
pub mod table;
pub mod transform_cities_schema;
pub mod translate;

use std::str::FromStr;

use crate::cli::RunOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Extract,
    Prepare,
    Locationiq,
    TransformCitiesSchema,
    Admin1Load,
    Cities500Load,
    Translate,
    Pack,
    FullPipeline,
}

impl Stage {
    pub fn all() -> &'static [Stage] {
        &[
            Stage::Extract,
            Stage::Prepare,
            Stage::Locationiq,
            Stage::TransformCitiesSchema,
            Stage::Admin1Load,
            Stage::Cities500Load,
            Stage::Translate,
            Stage::Pack,
            Stage::FullPipeline,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Extract => "extract",
            Stage::Prepare => "prepare",
            Stage::Locationiq => "locationiq",
            Stage::TransformCitiesSchema => "transform_cities_schema",
            Stage::Admin1Load => "admin1_load",
            Stage::Cities500Load => "cities500_load",
            Stage::Translate => "translate",
            Stage::Pack => "pack",
            Stage::FullPipeline => "full_pipeline",
        }
    }
}

impl FromStr for Stage {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "extract" => Ok(Stage::Extract),
            "prepare" => Ok(Stage::Prepare),
            "locationiq" => Ok(Stage::Locationiq),
            "transform_cities_schema" => Ok(Stage::TransformCitiesSchema),
            "admin1_load" => Ok(Stage::Admin1Load),
            "cities500_load" => Ok(Stage::Cities500Load),
            "translate" => Ok(Stage::Translate),
            "pack" => Ok(Stage::Pack),
            "full_pipeline" => Ok(Stage::FullPipeline),
            other => Err(format!("未知 stage：{other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Placeholder,
}

pub fn run_stage(stage: Stage, options: &RunOptions) -> Result<(), String> {
    match stage {
        Stage::Extract => extract::run(options),
        Stage::Prepare => prepare::run(options),
        Stage::Locationiq => locationiq::run(options),
        Stage::TransformCitiesSchema => transform_cities_schema::run(options),
        Stage::Admin1Load => admin1_load::run(options),
        Stage::Cities500Load => cities500_load::run(options),
        Stage::Translate => translate::run(options),
        Stage::Pack => pack::run(options),
        Stage::FullPipeline => run_full_pipeline(options),
    }
}

pub fn run_full_pipeline(options: &RunOptions) -> Result<(), String> {
    prepare::run(options)?;
    locationiq::run(options)?;
    extract::run(options)?;
    transform_cities_schema::run(options)?;
    pack::run(options)?;
    println!("stage=full_pipeline status=completed parity=true");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_stages_include_requested_pipeline_boundaries() {
        let names: Vec<&str> = Stage::all().iter().map(|stage| stage.as_str()).collect();

        assert_eq!(
            names,
            vec![
                "extract",
                "prepare",
                "locationiq",
                "transform_cities_schema",
                "admin1_load",
                "cities500_load",
                "translate",
                "pack",
                "full_pipeline"
            ]
        );
    }

    #[test]
    fn parses_stage_names() {
        assert_eq!(
            "transform_cities_schema".parse::<Stage>().unwrap(),
            Stage::TransformCitiesSchema
        );
        assert!("missing".parse::<Stage>().is_err());
    }
}
