//! Shared environment for building AWS Lambda (provided.al2023) Rust binaries.
//!
//! Lambda bootstraps must be linked against a glibc no newer than AL2023's
//! (2.34) with the standard `/lib/ld-linux-*.so.1` interpreter. On NixOS hosts
//! the default toolchain produces unusable binaries (Nix store INTERP / too-new
//! GLIBC). cargo-lambda + zig handles this when given a glibc-pinned target;
//! the util flake also wraps `cargo-lambda` to rewrite `--arm64`/`--x86-64`.

/// provided.al2023 glibc floor.
pub const LAMBDA_AL2023_GLIBC: &str = "2.34";

/// Arm64 Lambda target triple pinned to AL2023 glibc.
pub const LAMBDA_AARCH64_TARGET: &str = "aarch64-unknown-linux-gnu.2.34";

/// Env vars that should be present for every cargo-lambda / zig Lambda build.
///
/// Callers merge these with any additional vars (e.g. `SAM_BUILD_MODE`).
pub fn lambda_build_env() -> Vec<(String, String)> {
    vec![
        // Prefer zig cross-linking even when host arch matches Lambda.
        ("CARGO_LAMBDA_COMPILER".to_string(), "cargo-zigbuild".to_string()),
        // aws-lc-sys (rustls) under zig cc.
        ("AWS_LC_SYS_NO_JITTER_ENTROPY".to_string(), "1".to_string()),
        // Use cmake builder: the default cc builder runs a memcmp self-test when
        // HOST==TARGET, which false-fails on NixOS aarch64 (stub-ld cannot run
        // the zig-linked probe). Safe on macOS too.
        ("AWS_LC_SYS_CMAKE_BUILDER".to_string(), "1".to_string()),
        // Neutralize Nix gcc-wrapper rpath injection if present in the parent env.
        // Setting empty overrides inherited values (Command::envs merges over environ).
        ("NIX_LDFLAGS".to_string(), String::new()),
        ("NIX_CFLAGS_COMPILE".to_string(), String::new()),
        ("NIX_CFLAGS_LINK".to_string(), String::new()),
    ]
}
