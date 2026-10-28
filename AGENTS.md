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

Cargo workspace: `main/` is the façade crate `fractic-cli-scaffolding`,
re-exporting `lib_core` always and `lib_aws`, `lib_docker`, `lib_flutter`,
`lib_git`, `lib_image`, `lib_networking` behind the features `aws`,
`docker`, `flutter`, `git`, `image`, `networking`. `lib_core/src/` is
grouped by concern (`cli_error`, `common_entities`, `config`, `files`,
`general`, `processing`, `tty`, `user_input`); `lib_aws/src/` has one file
per AWS service.

- Test with `--workspace` from the root (through a product devshell).
- An AWS helper is a free `pub async fn` in `lib_aws/src/<service>.rs`
  (`pub mod` a new service in `lib_aws/src/lib.rs`); a terminal / file /
  input concern goes under the matching `lib_core` directory. Nothing
  consumer-specific (stack names, account ids, repo paths) belongs here;
  that is the consumer's `config.yaml`.
- Process execution is `tokio::process` through `Executor`
  (`ex.execute…`), never `std::process::Command`.
- A signature change ripples into every `util/` in the workspace: title it
  `Breaking: …`.
- Commit / PR titles: `[core] …`, `[aws] …`, `[networking] …`.
