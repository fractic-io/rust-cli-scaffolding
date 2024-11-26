use lib_core::{define_cli_error, CliError, Executor, IOMode, InvalidUTF8};
use openssh::{ForwardType, KnownHosts, SessionBuilder};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

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

pub fn wait_until_ssh_available(address: &str, port: u16) -> Result<(), CliError> {
    let address_with_port = format!("{}:{}", address, port);
    let timeout_duration = Duration::from_secs(5 * 60); // 5 minutes
    let start_time = Instant::now();

    let socket_addr: SocketAddr = address_with_port
        .parse()
        .map_err(|e| InvalidSshRequest::with_debug("could not parse address", &e))?;

    while start_time.elapsed() < timeout_duration {
        if TcpStream::connect_timeout(&socket_addr, Duration::from_secs(3)).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(1));
    }

    Err(SshWaitTimeout::new(timeout_duration.as_secs()))
}

pub async fn forward_port(
    address: &str,
    ssh_port: u16,
    direction: PortForward,
    forward_port: u16,
) -> Result<(), CliError> {
    let session = SessionBuilder::default()
        .connect(format!("{}:{}", address, ssh_port))
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
        .map_err(|e| SshPortForwardError::with_debug(forward_port, &e))
}

pub async fn ssh_cache_identity(
    ex: &Executor,
    identity_file: &PathBuf,
    ttl: Duration,
) -> Result<(), CliError> {
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
    Ok(())
}

/// Identity must be cached before calling this function.
pub async fn ssh_exec_command(
    address: &str,
    port: u16,
    identity_file: &PathBuf,
    program: &str,
    args: &[&str],
) -> Result<String, CliError> {
    let session = SessionBuilder::default()
        .known_hosts_check(KnownHosts::Add)
        .keyfile(identity_file)
        .connect(format!("{}:{}", address, port))
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

pub async fn ssh_attach(
    ex: &Executor,
    address: &str,
    port: u16,
    identity_file: &PathBuf,
    command: Option<&str>,
) -> Result<(), CliError> {
    let port = port.to_string();
    let identity_file = identity_file.display().to_string();

    let mut args = vec![
        "-p",
        &port,
        "-i",
        &identity_file,
        "-o",
        "StrictHostKeyChecking=accept-new",
        address,
    ];
    if let Some(command) = command {
        args.push("-t");
        args.push(command);
    }

    ex.execute("ssh", &args, None, IOMode::Attach)?;
    Ok(())
}
