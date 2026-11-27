mod account;
#[cfg(feature = "aws-acm")]
mod acm;
#[cfg(feature = "aws-autoscaling")]
mod autoscaling;
#[cfg(feature = "aws-lambda")]
mod cargo_lambda;
#[cfg(feature = "aws-cloudformation")]
mod cloudformation;
#[cfg(feature = "aws-cognito")]
mod cognito;
#[cfg(feature = "aws-ecr")]
mod ecr;
#[cfg(feature = "aws-ecs")]
mod ecs;
#[cfg(feature = "aws-iam")]
mod iam;
mod identity;
#[cfg(feature = "aws-lambda")]
mod lambda_build;
#[cfg(feature = "aws-logs")]
mod logs;
#[cfg(feature = "aws-route53")]
mod route53;
#[cfg(feature = "aws-route53domains")]
mod route53domains;
#[cfg(feature = "aws-s3")]
mod s3;
#[cfg(feature = "aws-lambda")]
mod sam;
#[cfg(feature = "aws-secrets")]
mod secrets;
#[cfg(feature = "aws-ses")]
mod ses;
mod shared_config;
#[cfg(feature = "aws-ssm")]
mod ssm;

pub use account::*;
#[cfg(feature = "aws-acm")]
pub use acm::*;
#[cfg(feature = "aws-autoscaling")]
pub use autoscaling::*;
#[cfg(feature = "aws-lambda")]
pub use cargo_lambda::*;
#[cfg(feature = "aws-cloudformation")]
pub use cloudformation::*;
#[cfg(feature = "aws-cognito")]
pub use cognito::*;
#[cfg(feature = "aws-ecr")]
pub use ecr::*;
#[cfg(feature = "aws-ecs")]
pub use ecs::*;
#[cfg(feature = "aws-iam")]
pub use iam::*;
pub use identity::*;
#[cfg(feature = "aws-lambda")]
pub use lambda_build::*;
#[cfg(feature = "aws-logs")]
pub use logs::*;
#[cfg(feature = "aws-route53")]
pub use route53::*;
#[cfg(feature = "aws-route53domains")]
pub use route53domains::*;
#[cfg(feature = "aws-s3")]
pub use s3::*;
#[cfg(feature = "aws-lambda")]
pub use sam::*;
#[cfg(feature = "aws-secrets")]
pub use secrets::*;
#[cfg(feature = "aws-ses")]
pub use ses::*;
pub use shared_config::*;
#[cfg(feature = "aws-ssm")]
pub use ssm::*;
