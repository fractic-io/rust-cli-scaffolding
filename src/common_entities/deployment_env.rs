use clap::ValueEnum;
use fractic_core::impl_deterministic_display_from_serde;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, EnumIter, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "snake_case")]
pub enum DeploymentEnv {
    Sandbox,
    Staging,
    Production,
}

impl_deterministic_display_from_serde!(DeploymentEnv);
