use futures_util::stream::{self, StreamExt as _, TryStreamExt as _};
use lib_core::{define_cli_error, CliError, CriticalError, Executor, IOMode, Printer};
use std::path::PathBuf;

use crate::{ssh_exec_command, SshConnectOptions};

define_cli_error!(
    InvalidScpRequest,
    "Failed to send SCP request: {details}.",
    { details: &str }
);
define_cli_error!(
    SshRemoteFileInfoParseError,
    "Failed to parse remote file info line: '{line}'.",
    { line: &str }
);

#[derive(Debug, Clone)]
pub struct SshRemoteFileInfo {
    pub path: String,
    pub size: u64,
    pub modified_epoch_sec: i64,
}

pub fn scp_upload_file<'a>(
    pr: &Printer,
    ex: &Executor,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    file: &PathBuf,
    destination: &str,
) -> Result<(), CliError> {
    scp_upload_files(
        pr,
        ex,
        user,
        hostname,
        connect_options,
        vec![file],
        destination,
    )
}

pub fn scp_upload_files<'a>(
    pr: &Printer,
    ex: &Executor,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    files: Vec<&PathBuf>,
    destination: &str,
) -> Result<(), CliError> {
    let connect_opt = connect_options.unwrap_or_default();
    let port = connect_opt.port_or_default().to_string();
    let identity_file = connect_opt.identity_file_or_default();
    let known_hosts_file = connect_opt.known_hosts_file_or_default();
    let connect_timeout = connect_opt.connect_timeout_or_default();
    let known_hosts_opt = format!("UserKnownHostsFile={}", known_hosts_file);
    let connect_timeout_opt = format!("ConnectTimeout={}", connect_timeout.as_secs());

    let scp_sources = files
        .iter()
        .map(|f| {
            f.to_str().ok_or_else(|| {
                CriticalError::new(&format!(
                    "failed to convert path '{}' to string.",
                    f.display()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scp_dest = format!("{}@{}:{}", user, hostname, destination);

    match scp_sources.len() {
        0 => {
            pr.warn("Skipping scp command (no files to copy)...");
            return Ok(());
        }
        1 => {
            pr.info(&format!(
                "Copying '{}' to '{}'...",
                scp_sources[0], scp_dest
            ));
        }
        _ => {
            pr.info(&format!(
                "Copying {} files to '{}'...",
                scp_sources.len(),
                scp_dest
            ));
        }
    }

    let mut args = vec![
        "-P",
        &port,
        "-i",
        &identity_file,
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        &known_hosts_opt,
        "-o",
        &connect_timeout_opt,
    ];
    args.extend(scp_sources);
    args.push(&scp_dest);

    ex.execute("scp", &args, IOMode::Silent)?;

    Ok(())
}

pub fn scp_upload_dir<'a>(
    pr: &Printer,
    ex: &Executor,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    dir: &PathBuf,
    destination: &str,
) -> Result<(), CliError> {
    let files = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    scp_upload_files(
        pr,
        ex,
        user,
        hostname,
        connect_options,
        files.iter().collect(),
        destination,
    )
}

pub async fn scp_list_files_recursive<'a>(
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    path: &str,
) -> Result<Vec<SshRemoteFileInfo>, CliError> {
    let output = ssh_exec_command(
        user,
        hostname,
        connect_options,
        "sh",
        &[
            "-lc",
            "p=\"$1\"; if [ -d \"$p\" ]; then find \"$p\" -type f -printf '%T@|%s|%p\\n'; elif [ \
             -f \"$p\" ]; then stat -c '%Y|%s|%n' \"$p\"; fi",
            "sh",
            path,
        ],
    )
    .await?;

    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.splitn(3, '|');
            let modified_raw = parts
                .next()
                .ok_or_else(|| SshRemoteFileInfoParseError::new(line))?;
            let size_raw = parts
                .next()
                .ok_or_else(|| SshRemoteFileInfoParseError::new(line))?;
            let path_raw = parts
                .next()
                .ok_or_else(|| SshRemoteFileInfoParseError::new(line))?;

            let modified_epoch_sec = if modified_raw.contains('.') {
                modified_raw
                    .split('.')
                    .next()
                    .unwrap_or_default()
                    .parse::<i64>()
                    .map_err(|e| SshRemoteFileInfoParseError::with_debug(line, &e))?
            } else {
                modified_raw
                    .parse::<i64>()
                    .map_err(|e| SshRemoteFileInfoParseError::with_debug(line, &e))?
            };

            let size = size_raw
                .parse::<u64>()
                .map_err(|e| SshRemoteFileInfoParseError::with_debug(line, &e))?;

            Ok(SshRemoteFileInfo {
                path: path_raw.to_string(),
                size,
                modified_epoch_sec,
            })
        })
        .collect()
}

pub fn scp_download_file<'a>(
    ex: &Executor,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    remote_path: &str,
    local_path: &str,
) -> Result<(), CliError> {
    let connect_opt = connect_options.unwrap_or_default();
    let port = connect_opt.port_or_default().to_string();
    let identity_file = connect_opt.identity_file_or_default();
    let known_hosts_file = connect_opt.known_hosts_file_or_default();
    let connect_timeout = connect_opt.connect_timeout_or_default();
    let known_hosts_opt = format!("UserKnownHostsFile={}", known_hosts_file);
    let connect_timeout_opt = format!("ConnectTimeout={}", connect_timeout.as_secs());
    let source = format!("{}@{}:{}", user, hostname, remote_path);

    let args = vec![
        "-P",
        &port,
        "-i",
        &identity_file,
        "-p",
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        &known_hosts_opt,
        "-o",
        &connect_timeout_opt,
        &source,
        local_path,
    ];
    ex.execute("scp", &args, IOMode::Silent)?;

    Ok(())
}

pub async fn scp_download_files<'a>(
    pr: &Printer,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    files: Vec<(String, String)>,
    max_concurrency: usize,
) -> Result<(), CliError> {
    if files.is_empty() {
        return Ok(());
    }

    pr.info(&format!(
        "Downloading {} file{} via SCP from '{}@{}'...",
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        user,
        hostname
    ));

    let connect_opt = connect_options.unwrap_or_default();
    let port = connect_opt.port_or_default().to_string();
    let identity_file = connect_opt.identity_file_or_default();
    let known_hosts_file = connect_opt.known_hosts_file_or_default();
    let connect_timeout = connect_opt.connect_timeout_or_default();
    let known_hosts_opt = format!("UserKnownHostsFile={}", known_hosts_file);
    let connect_timeout_opt = format!("ConnectTimeout={}", connect_timeout.as_secs());

    stream::iter(files.into_iter().map(|(remote_path, local_path)| {
        let port = port.clone();
        let identity_file = identity_file.clone();
        let known_hosts_opt = known_hosts_opt.clone();
        let connect_timeout_opt = connect_timeout_opt.clone();
        let source = format!("{}@{}:{}", user, hostname, remote_path);
        async move {
            let out = tokio::process::Command::new("scp")
                .args([
                    "-P",
                    &port,
                    "-i",
                    &identity_file,
                    "-p",
                    "-o",
                    "StrictHostKeyChecking=accept-new",
                    "-o",
                    &known_hosts_opt,
                    "-o",
                    &connect_timeout_opt,
                    &source,
                    &local_path,
                ])
                .output()
                .await
                .map_err(|e| InvalidScpRequest::with_debug("failed to execute", &e))?;
            if out.status.success() {
                Ok::<(), CliError>(())
            } else {
                Err(InvalidScpRequest::new("returned non-zero status"))
            }
        }
    }))
    .buffer_unordered(max_concurrency.max(1))
    .try_collect::<Vec<_>>()
    .await?;

    Ok(())
}

pub async fn scp_delete_file<'a>(
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    path: &str,
) -> Result<(), CliError> {
    let _ = ssh_exec_command(
        user,
        hostname,
        connect_options,
        "sh",
        &["-lc", "rm -f -- \"$1\"", "sh", path],
    )
    .await?;

    Ok(())
}

pub async fn scp_delete_files<'a>(
    pr: &Printer,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    paths: Vec<String>,
    max_concurrency: usize,
) -> Result<(), CliError> {
    if paths.is_empty() {
        return Ok(());
    }

    pr.info(&format!(
        "Deleting {} file{} via SCP from '{}@{}'...",
        paths.len(),
        if paths.len() == 1 { "" } else { "s" },
        user,
        hostname
    ));

    stream::iter(
        paths.into_iter().map(|path| async move {
            scp_delete_file(user, hostname, connect_options, &path).await
        }),
    )
    .buffer_unordered(max_concurrency.max(1))
    .try_collect::<Vec<_>>()
    .await?;

    Ok(())
}
