use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, EnumIter, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "snake_case")]
pub enum DeploymentColor {
    Blue,
    Green,
}

impl std::fmt::Display for DeploymentColor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        serde::Serialize::serialize(self, f)
    }
}
