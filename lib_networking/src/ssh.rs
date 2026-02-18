use chrono::{Datelike, Local, NaiveDate, NaiveTime};
use lib_core::{define_cli_error, CliError, CriticalError, Executor, IOMode, InvalidUTF8, Printer};
use nix::unistd;
use openssh::{ForwardType, KnownHosts, Session, SessionBuilder};
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{close_open_sockets_on_port, wait_until_socket_open};

define_cli_error!(
    InvalidSshRequest,
    "Failed to send SSH request: {details}.",
    { details: &str }
);
define_cli_error!(
    SshWaitTimeout,
    "SSH server did not become available within timeout of {timeout_sec}s.",
    { timeout_sec: u64 }
);
define_cli_error!(
    SshConnectionError,
    "Failed to establish a connection to the SSH server."
);
define_cli_error!(
    SshPortForwardError,
    "Failed to forward port {port}.",
    { port: u16 }
);
define_cli_error!(
    SshfsPermissionError,
    "User does not have write permissions to the local path: {path}.",
    { path: &str }
);

#[derive(Debug)]
pub enum PortForward {
    Local,
    Remote,
}

impl From<PortForward> for ForwardType {
    fn from(pf: PortForward) -> ForwardType {
        match pf {
            PortForward::Local => ForwardType::Local,
            PortForward::Remote => ForwardType::Remote,
        }
    }
}

pub struct PortForwardHandle {
    _session: Session,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SshConnectOptions<'a> {
    pub port: Option<u16>,
    pub identity_file: Option<&'a PathBuf>,
    pub known_hosts_file: Option<&'a PathBuf>,
    pub connect_timeout: Option<Duration>,
}

impl<'a> SshConnectOptions<'a> {
    pub fn port_or_default(&self) -> u16 {
        self.port.unwrap_or(22)
    }

    pub fn identity_file_or_default(&self) -> String {
        self.identity_file
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.ssh/id_rsa".to_string())
    }

    pub fn known_hosts_file_or_default(&self) -> String {
        self.known_hosts_file
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.ssh/known_hosts".to_string())
    }

    pub fn connect_timeout_or_default(&self) -> Duration {
        self.connect_timeout.unwrap_or(Duration::from_secs(10))
    }
}

#[derive(Debug, Default)]
pub struct SshAttachOptions<'a> {
    pub command: Option<&'a str>,
    pub inactivity_timeout: Option<Duration>,
}

#[derive(Debug, Clone, Copy)]
pub enum SshCacheTtl {
    /// Cache identity for a specific duration.
    For(Duration),
    /// Cache identity until a given local time-of-day.
    Until(NaiveTime),
    /// Cache identity until 4AM.
    UntilEOD,
}

/// Returns the IP address the hostname resolves to once it becomes available.
pub async fn wait_until_ssh_available<'a>(
    pr: &mut Printer,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
) -> Result<String, CliError> {
    let port = connect_options
        .as_ref()
        .and_then(|co| co.port)
        .unwrap_or(22);

    // First, wait for socket to be open.
    let ip = wait_until_socket_open(pr, hostname, port).await?;

    // Next, wait for SSH server to be available.
    let timeout_duration = Duration::from_secs(10 * 60); // 10 minutes
    let start_time = Instant::now();
    pr.with_status_bar(|mut status_bar| async move {
        let mut last_error = None;
        while start_time.elapsed() < timeout_duration {
            match ssh_exec_command(user, hostname, connect_options, "echo", &["Connected."]).await {
                Ok(_) => {
                    status_bar.important("Connected.");
                    return Ok(ip);
                }
                Err(e) => {
                    status_bar.info(&format!("{}; {}", e.message(), "Retrying..."));
                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
        Err(SshWaitTimeout::with_debug(
            timeout_duration.as_secs(),
            &last_error,
        ))
    })
    .await
}

/// The port forwarding remains active until the returned PortForwardHandle is dropped.
pub async fn forward_port<'a>(
    pr: &Printer,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    direction: PortForward,
    forward_port: u16,
    force_close_existing: bool,
) -> Result<PortForwardHandle, CliError> {
    match direction {
        PortForward::Local => pr.info(&format!(
            "Forwarding '{}:{}' to localhost...",
            hostname, forward_port
        )),
        PortForward::Remote => pr.info(&format!(
            "Forwarding localhost:{} to '{}'...",
            forward_port, hostname
        )),
    }

    let connect_opt = connect_options.unwrap_or_default();
    let ssh_port = connect_opt.port_or_default().to_string();
    let identity_file = connect_opt.identity_file_or_default();
    let known_hosts_file = connect_opt.known_hosts_file_or_default();
    let connect_timeout = connect_opt.connect_timeout_or_default();

    if force_close_existing {
        close_open_sockets_on_port(pr, forward_port)?;
    }

    let session = SessionBuilder::default()
        .known_hosts_check(KnownHosts::Add)
        .keyfile(identity_file)
        .user_known_hosts_file(known_hosts_file)
        .connect_timeout(connect_timeout)
        .connect(format!("ssh://{}@{}:{}", user, hostname, ssh_port))
        .await
        .map_err(|e| SshConnectionError::with_debug(&e))?;

    session
        .request_port_forward(
            direction,
            (
                std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                forward_port,
            ),
            (
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                forward_port,
            ),
        )
        .await
        .map_err(|e| SshPortForwardError::with_debug(forward_port, &e))?;

    Ok(PortForwardHandle { _session: session })
}

pub fn ssh_cache_identity(
    pr: &Printer,
    ex: &Executor,
    identity_file: &PathBuf,
    ttl: SshCacheTtl,
) -> Result<(), CliError> {
    fn duration_until_time_of_day(target: NaiveTime) -> Duration {
        let now = Local::now();
        let today_date = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day())
            .expect("valid current date");
        let today_target = today_date.and_time(target);
        let now_naive = now.naive_local();

        let delta = if today_target > now_naive {
            today_target - now_naive
        } else {
            let tomorrow_date = today_date.succ_opt().expect("valid next day");
            let tomorrow_target = tomorrow_date.and_time(target);
            tomorrow_target - now_naive
        };

        // Ensure non-zero positive duration.
        delta
            .to_std()
            .unwrap_or_else(|_| Duration::from_secs(1))
            .max(Duration::from_secs(1))
    }

    let ttl = match ttl {
        SshCacheTtl::For(d) => d,
        SshCacheTtl::Until(time_of_day) => duration_until_time_of_day(time_of_day),
        SshCacheTtl::UntilEOD => duration_until_time_of_day(
            NaiveTime::from_hms_opt(4, 0, 0).expect("hardcoded NaiveTime should be valid"),
        ),
    };
    let agent_init = ex.execute("ssh-agent", &["-s"], IOMode::Silent)?;
    ex.execute("sh", &["-c", &agent_init], IOMode::Silent)?;

    let existing_cached_identities = ex
        .execute("ssh-add", &["-l"], IOMode::Silent)
        // NOTE: 'ssh-add -l' returns error code 1 if the agent has no
        // identities, so just treat an error as empty.
        .unwrap_or_default();
    let search_query = ex.execute(
        "ssh-keygen",
        &["-lf", &identity_file.display().to_string()],
        IOMode::Silent,
    )?;
    let search_query_sha_component = search_query.split_whitespace().nth(1).ok_or_else(|| {
        CriticalError::new(&format!(
            "could not find SHA component of ssh-keygen spec: '{}'.",
            search_query
        ))
    })?;

    if !existing_cached_identities.contains(search_query_sha_component) {
        pr.info(&format!(
            "Caching SSH identity file '{}'...",
            identity_file.display()
        ));
        ex.execute(
            "ssh-add",
            &[
                "-t",
                &ttl.as_secs().to_string(),
                &identity_file.display().to_string(),
            ],
            IOMode::Attach,
        )?;
    }
    Ok(())
}

/// Identity must be cached before calling this function.
pub async fn ssh_exec_command<'a>(
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    program: &str,
    args: &[&str],
) -> Result<String, CliError> {
    let connect_opt = connect_options.unwrap_or_default();
    let port = connect_opt.port_or_default().to_string();
    let identity_file = connect_opt.identity_file_or_default();
    let known_hosts_file = connect_opt.known_hosts_file_or_default();
    let connect_timeout = connect_opt.connect_timeout_or_default();

    let session = SessionBuilder::default()
        .known_hosts_check(KnownHosts::Add)
        .keyfile(identity_file)
        .user_known_hosts_file(known_hosts_file)
        .connect_timeout(connect_timeout)
        .connect(format!("ssh://{}@{}:{}", user, hostname, port))
        .await
        .map_err(|e| SshConnectionError::with_debug(&e))?;

    let out = session
        .command(program)
        .args(args)
        .output()
        .await
        .map_err(|e| SshConnectionError::with_debug(&e))?;

    String::from_utf8(out.stdout).map_err(|e| InvalidUTF8::with_debug(&e))
}

pub fn ssh_attach<'a>(
    ex: &Executor,
    user: &str,
    hostname: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    attach_options: Option<SshAttachOptions>,
) -> Result<(), CliError> {
    let connect_opt = connect_options.unwrap_or_default();
    let attach_opt = attach_options.unwrap_or_default();

    let port = connect_opt.port_or_default().to_string();
    let identity_file = connect_opt.identity_file_or_default();
    let known_hosts_file = connect_opt.known_hosts_file_or_default();
    let connect_timeout = connect_opt.connect_timeout_or_default();
    let known_hosts_opt = format!("UserKnownHostsFile={}", known_hosts_file);
    let connect_timeout_opt = format!("ConnectTimeout={}", connect_timeout.as_secs());
    let address = format!("{}@{}", user, hostname);

    let command = match (attach_opt.inactivity_timeout, attach_opt.command) {
        (Some(timeout), Some(command)) => {
            format!(
                "exec $SHELL -lic 'timeout {}s {}'",
                timeout.as_secs(),
                command
            )
        }
        (Some(timeout), None) => format!("export TMOUT={}; exec $SHELL -l", timeout.as_secs()),
        (None, Some(command)) => {
            format!("exec $SHELL -lic '{}'", command)
        }
        (None, None) => "exec $SHELL -l".to_string(),
    };

    let args = vec![
        "-p",
        &port,
        "-i",
        &identity_file,
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        &known_hosts_opt,
        "-o",
        &connect_timeout_opt,
        &address,
        "-t",
        &command,
    ];

    ex.execute("ssh", &args, IOMode::Attach)?;
    Ok(())
}

pub fn sshfs<'a>(
    ex: &mut Executor,
    remote_path: &str,
    local_path: &str,
    connect_options: Option<SshConnectOptions<'a>>,
    sudo_fallback: bool,
) -> Result<(), CliError> {
    let connect_opt = connect_options.unwrap_or_default();
    let port = connect_opt.port_or_default().to_string();
    let known_hosts_file = connect_opt.known_hosts_file_or_default();
    let identity_file = connect_opt.identity_file_or_default();
    let connect_timeout = connect_opt.connect_timeout_or_default();
    let known_hosts_opt = format!("UserKnownHostsFile={}", known_hosts_file);
    let identity_file_opt = format!("IdentityFile={}", identity_file);
    let connect_timeout_opt = format!("ConnectTimeout={}", connect_timeout.as_secs());

    let user_has_write_permissions = match fs::metadata(local_path) {
        Ok(meta) => {
            let uid = meta.uid();
            let mode = meta.mode();
            let mine = uid == unistd::geteuid().as_raw();
            let write = mode & 0o200 != 0;
            mine && write
        }
        _ => false,
    };

    let common_args = {
        let mut args = vec![
            "-f", // Foreground.
            "-p", // Change port.
            &port,
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            &known_hosts_opt,
            "-o",
            &identity_file_opt,
            "-o",
            &connect_timeout_opt,
        ];
        if cfg!(target_os = "macos") {
            // Prevent .DS_Store & ._* files:
            args.extend_from_slice(&["-o", "noappledouble"]);
        }
        args
    };

    if user_has_write_permissions {
        // Run as local user.
        let mut args = vec![];
        args.extend_from_slice(&common_args);
        args.extend_from_slice(&["-o", "idmap=user", remote_path, local_path]);
        ex.execute_background("sshfs", &args, None)?;
        Ok(())
    } else if sudo_fallback {
        // Run as root, but mount as local user.
        let user_override = format!(
            "uid={},gid={}",
            unistd::geteuid().as_raw(),
            unistd::getegid().as_raw()
        );
        let mut args = vec!["sshfs"];
        args.extend_from_slice(&common_args);
        args.extend_from_slice(&[
            "-o",
            "allow_other",
            "-o",
            &user_override,
            remote_path,
            local_path,
        ]);
        ex.execute_background("sudo", &args, None)?;
        Ok(())
    } else {
        Err(SshfsPermissionError::new(local_path))
    }
}
