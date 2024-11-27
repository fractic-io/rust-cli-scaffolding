use lib_core::{define_cli_error, CliError, CriticalError, Executor, IOMode, InvalidUTF8, Printer};
use openssh::{ForwardType, KnownHosts, Session, SessionBuilder};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::dns_query_a_record;

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

/// Returns the IP address the hostname resolves to once it becomes available.
pub async fn wait_until_ssh_available(
    pr: &Printer,
    hostname: &str,
    port: u16,
) -> Result<String, CliError> {
    pr.info(&format!(
        "Waiting for '{}:{}' to become available...",
        hostname, port
    ));

    let timeout_duration = Duration::from_secs(5 * 60); // 5 minutes
    let start_time = Instant::now();

    while start_time.elapsed() < timeout_duration {
        let ip_addr = if is_ip_address(hostname) {
            hostname.to_string()
        } else {
            match dns_query_a_record(hostname).await {
                Ok(ip) => ip,
                Err(e) => {
                    pr.warn(&format!(
                        "WARNING: Failed to resolve hostname '{}'. {}",
                        hostname,
                        e.message()
                    ));
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }
        };

        let address_with_port = format!("{}:{}", ip_addr, port);
        let socket_addr: SocketAddr = address_with_port
            .parse()
            .map_err(|e| InvalidSshRequest::with_debug("could not parse address", &e))?;
        if TcpStream::connect_timeout(&socket_addr, Duration::from_secs(3)).is_ok() {
            return Ok(ip_addr);
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Err(SshWaitTimeout::new(timeout_duration.as_secs()))
}

/// The port forwarding remains active until the returned PortForwardHandle is dropped.
pub async fn forward_port(
    pr: &Printer,
    user: &str,
    hostname: &str,
    ssh_port: u16,
    identity_file: &PathBuf,
    direction: PortForward,
    forward_port: u16,
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

    let session = SessionBuilder::default()
        .known_hosts_check(KnownHosts::Add)
        .keyfile(identity_file)
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
pub async fn ssh_exec_command(
    user: &str,
    hostname: &str,
    port: u16,
    identity_file: &PathBuf,
    program: &str,
    args: &[&str],
) -> Result<String, CliError> {
    let session = SessionBuilder::default()
        .known_hosts_check(KnownHosts::Add)
        .keyfile(identity_file)
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

pub fn ssh_attach(
    ex: &Executor,
    user: &str,
    hostname: &str,
    port: u16,
    identity_file: &PathBuf,
    command: Option<&str>,
) -> Result<(), CliError> {
    let port = port.to_string();
    let identity_file = identity_file.display().to_string();
    let address = format!("{}@{}", user, hostname);

    let mut args = vec![
        "-p",
        &port,
        "-i",
        &identity_file,
        "-o",
        "StrictHostKeyChecking=accept-new",
        &address,
    ];
    if let Some(command) = command {
        args.push("-t");
        args.push(command);
    }

    ex.execute("ssh", &args, None, IOMode::Attach)?;
    Ok(())
}

fn is_ip_address(address: &str) -> bool {
    address.parse::<std::net::IpAddr>().is_ok()
}
