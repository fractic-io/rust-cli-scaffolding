use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentColor {
    Blue,
    Green,
}

impl std::fmt::Display for DeploymentColor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        serde::Serialize::serialize(self, f)
    }
}
