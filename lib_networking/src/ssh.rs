use lib_core::{define_cli_error, CliError, CriticalError, Executor, IOMode, InvalidUTF8, Printer};
use openssh::{ForwardType, KnownHosts, Session, SessionBuilder};
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

#[derive(Debug, Default)]
pub struct SshAttachOptions<'a> {
    pub command: Option<&'a str>,
    pub inactivity_timeout: Option<Duration>,
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
    let ssh_port = connect_opt.port.unwrap_or(22).to_string();
    let identity_file = connect_opt
        .identity_file
        .map_or_else(|| "~/.ssh/id_rsa".to_string(), |p| p.display().to_string());
    let known_hosts_file = connect_opt.known_hosts_file.map_or_else(
        || "~/.ssh/known_hosts".to_string(),
        |p| p.display().to_string(),
    );
    let connect_timeout = connect_opt
        .connect_timeout
        .unwrap_or(Duration::from_secs(10));

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
                std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
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
    ttl: Duration,
) -> Result<(), CliError> {
    let agent_init = ex.execute("ssh-agent", &["-s"], None, IOMode::Silent)?;
    ex.execute("sh", &["-c", &agent_init], None, IOMode::Silent)?;

    let existing_cached_identities = ex
        .execute("ssh-add", &["-l"], None, IOMode::Silent)
        // NOTE: 'ssh-add -l' returns error code 1 if the agent has no
        // identities, so just treat an error as empty.
        .unwrap_or_default();
    let search_query = ex.execute(
        "ssh-keygen",
        &["-lf", &identity_file.display().to_string()],
        None,
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
            None,
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
    let port = connect_opt.port.unwrap_or(22).to_string();
    let identity_file = connect_opt
        .identity_file
        .map_or_else(|| "~/.ssh/id_rsa".to_string(), |p| p.display().to_string());
    let known_hosts_file = connect_opt.known_hosts_file.map_or_else(
        || "~/.ssh/known_hosts".to_string(),
        |p| p.display().to_string(),
    );
    let connect_timeout = connect_opt
        .connect_timeout
        .unwrap_or(Duration::from_secs(10));

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

    let port = connect_opt.port.unwrap_or(22).to_string();
    let identity_file = connect_opt
        .identity_file
        .map_or_else(|| "~/.ssh/id_rsa".to_string(), |p| p.display().to_string());
    let known_hosts_file = connect_opt.known_hosts_file.map_or_else(
        || "~/.ssh/known_hosts".to_string(),
        |p| p.display().to_string(),
    );
    let connect_timeout = connect_opt
        .connect_timeout
        .unwrap_or(Duration::from_secs(10));
    let known_hosts_opt = format!("UserKnownHostsFile={}", known_hosts_file);
    let connect_timeout_opt = format!("ConnectTimeout={}", connect_timeout.as_secs());
    let address = format!("{}@{}", user, hostname);

    let command = match (attach_opt.inactivity_timeout, attach_opt.command) {
        (Some(timeout), Some(command)) => {
            format!("timeout {}s {}", timeout.as_secs(), command)
        }
        (Some(timeout), None) => format!("export TMOUT={}; exec $SHELL -l", timeout.as_secs()),
        (None, Some(command)) => command.to_string(),
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

    ex.execute("ssh", &args, None, IOMode::Attach)?;
    Ok(())
}
