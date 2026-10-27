# fractic-cli-scaffolding

The toolkit every `util/` operator CLI and the storyengine CLI are built on:
`Tty` (sections, printer, executor, user preferences in `~/.fractic_cli.yaml`),
`CliError` + `define_cli_error!`, user input (basic prompts, `Selectable`
selectors, vim editing), files (AES-GCM encryption, placeholders, tar,
temporary dirs), `DeploymentEnv` / `DeploymentColor`,
`define_parameterized_string!`, and feature-gated helpers for AWS
(CloudFormation, SAM, cargo-lambda, Cognito, S3, Secrets, SSM, Route53, ECS,
…), Docker, Flutter / Android, git, images and networking (SSH, SCP, DNS,
sockets). Public, provided as-is (`fractic-io/rust-cli-scaffolding`,
identity `fractic`).

## Read first

- `~/.agents/AGENTS.md` (workspace orientation; loaded automatically by Claude
  Code, read it if you are Codex or Grok), `~/.agents/guides/rust.md` (style)
  and `~/.agents/guides/repo.md` §2 (how the operator CLIs that consume this
  crate are laid out).

## Layout

Cargo workspace: `main/` is the façade crate `fractic-cli-scaffolding` that
re-exports `lib_core` always and `lib_aws`, `lib_docker`, `lib_flutter`,
`lib_git`, `lib_image`, `lib_networking` behind the features `aws`, `docker`,
`flutter`, `git`, `image`, `networking`. `lib_core/src/` is grouped by
concern (`cli_error`, `common_entities`, `config`, `files`, `general`,
`processing`, `tty`, `user_input`), each a directory with `mod.rs` re-exports
or a single file. `lib_aws/src/` has one file per AWS service.

## Working here

- Build with the workspace at the root (`Cargo.toml` lists the members); no
  flake here, use a product devshell: `nix develop
  /efs/fractic/storyengine/util --command cargo test --workspace`.
- Add an AWS helper as a free `pub async fn` in `lib_aws/src/<service>.rs`
  (create the file and `pub mod` it in `lib_aws/src/lib.rs` for a new
  service); add a terminal / file / input concern under the matching
  `lib_core` directory. Nothing consumer-specific (stack names, account ids,
  repo paths) belongs here; that is the consumer's `config.yaml`.
- Process execution is `tokio::process` through `Executor`
  (`ex.execute…`), never `std::process::Command`.
- Consumers `[patch]` this crate; a signature change ripples into every
  `util/` in the workspace, so title it `Breaking: …`.
- Commit / PR titles: `[core] …`, `[aws] …`, `[networking] …`, imperative,
  trailing period.
