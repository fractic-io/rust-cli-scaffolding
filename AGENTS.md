# fractic-cli-scaffolding

The toolkit for operator and terminal CLIs: `Tty` (sections, printer,
executor, user preferences in `~/.fractic_cli.yaml`), `CliError` +
`define_cli_error!`, user input (basic prompts, `Selectable` selectors, vim
editing), files (AES-GCM encryption, placeholders, tar, temporary dirs),
`DeploymentEnv` / `DeploymentColor`, `define_parameterized_string!`, and
feature-gated helpers for AWS (CloudFormation, SAM, cargo-lambda, Cognito,
S3, Secrets, SSM, Route53, ECS, …), Docker, Flutter / Android, git, images
and networking (SSH, SCP, DNS, sockets).

One crate. `src/lib.rs` re-exports the core modules (`cli_error`,
`common_entities`, `config`, `files`, `general`, `processing`, `tty`,
`user_input`) always and `aws`, `docker`, `flutter`, `git`, `image`,
`networking` behind the features of the same names. A feature module's
dependencies are optional and enabled by its feature in `Cargo.toml`.
`src/aws/` has one file per AWS service, each behind its own `aws-<service>`
feature (`src/aws/mod.rs`), so that a consumer compiles only the SDK crates
of the services it calls; `aws` is all of them.

- Test with `--all-features`, and `cargo check --features <f>` for each
  feature alone when a dependency moves. `image` needs ImageMagick 7
  (MagickWand) and libclang at build time, for `magick_rust`'s bindgen.
- An AWS helper is a free `pub async fn` in `src/aws/<service>.rs`. A new
  service gets a `mod` and `pub use` in `src/aws/mod.rs` behind a new
  `aws-<service>` feature, which `aws` lists; a terminal / file / input
  concern goes under the matching core directory in `src/`. Nothing
  consumer-specific (stack names, account ids, repo paths) belongs here;
  that is the consumer's configuration.
- Process execution is `tokio::process` through `Executor`
  (`ex.execute…`), never `std::process::Command`.
