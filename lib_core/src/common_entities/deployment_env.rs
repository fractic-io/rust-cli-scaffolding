use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentEnv {
    Sandbox,
    Staging,
    Production,
}

impl std::fmt::Display for DeploymentEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        serde::Serialize::serialize(self, f)
    }
}
