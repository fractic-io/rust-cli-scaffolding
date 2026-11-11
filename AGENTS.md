# fractic-cli-scaffolding

The toolkit every `util/` operator CLI and the storyengine CLI are built on:
`Tty` (sections, printer, executor, user preferences in
`~/.fractic_cli.yaml`), `CliError` + `define_cli_error!`, user input (basic
prompts, `Selectable` selectors, vim editing), files (AES-GCM encryption,
placeholders, tar, temporary dirs), `DeploymentEnv` / `DeploymentColor`,
`define_parameterized_string!`, and feature-gated helpers for AWS
(CloudFormation, SAM, cargo-lambda, Cognito, S3, Secrets, SSM, Route53, ECS,
…), Docker, Flutter / Android, git, images and networking (SSH, SCP, DNS,
sockets). Consumer side: `/efs/fractic/_/repo.md` §2.

One crate. `src/lib.rs` re-exports the core modules (`cli_error`,
`common_entities`, `config`, `files`, `general`, `processing`, `tty`,
`user_input`) always and `aws`, `docker`, `flutter`, `git`, `image`,
`networking` behind the features of the same names. A feature module's
dependencies are optional and enabled by its feature in `Cargo.toml`;
`src/aws/` has one file per AWS service.

- Test with `--all-features`, and `cargo check --features <f>` for each
  feature alone when a dependency moves (through a product devshell; the
  frontend `util/` one has ImageMagick for `image`).
- An AWS helper is a free `pub async fn` in `src/aws/<service>.rs` (`mod` and
  `pub use` a new service in `src/aws/mod.rs`); a terminal / file / input
  concern goes under the matching core directory in `src/`. Nothing
  consumer-specific (stack names, account ids, repo paths) belongs here;
  that is the consumer's `config.yaml`.
- Process execution is `tokio::process` through `Executor`
  (`ex.execute…`), never `std::process::Command`.
- A signature change ripples into every `util/` in the workspace: mark the
  PR `↯`.
