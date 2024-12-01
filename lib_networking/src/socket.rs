use std::{
    net::{SocketAddr, TcpStream},
    process::Command,
    time::{Duration, Instant},
};

use lib_core::{define_cli_error, CliError, Printer};
use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};

use crate::dns_query_a_record;

define_cli_error!(
    GetSocketInfoError,
    "Failed to get info on currently open sockets."
);
define_cli_error!(
    FailedToCloseSocket,
    "Failed to kill PID {pid} to free port {port}.",
    { pid: u32, port: u16 }
);
define_cli_error!(
    InvalidSocketAddress,
    "Failed to parse socket address: {address}.",
    { address: &str }
);
define_cli_error!(
    SocketWaitTimeout,
    "Socket did not become available within timeout of {timeout_sec}s.",
    { timeout_sec: u64 }
);

pub fn close_open_sockets_on_port(pr: &Printer, port: u16) -> Result<(), CliError> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP;

    let sockets_info = netstat2::get_sockets_info(af_flags, proto_flags)
        .map_err(|e| GetSocketInfoError::with_debug(&e))?;
    for socket in sockets_info {
        if let ProtocolSocketInfo::Tcp(tcp_socket) = socket.protocol_socket_info {
            if tcp_socket.local_port == port {
                if let Some(pid) = socket.associated_pids.get(0) {
                    pr.warn(&format!(
                        "WARNING: Closing existing connection on port {port} (PID: {pid})...",
                    ));

                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    {
                        Command::new("kill")
                            .arg("-9")
                            .arg(format!("{}", pid))
                            .output()
                            .map_err(|e| FailedToCloseSocket::with_debug(*pid, port, &e))?;
                    }

                    #[cfg(target_os = "windows")]
                    {
                        Command::new("taskkill")
                            .arg("/PID")
                            .arg(format!("{}", pid))
                            .arg("/F")
                            .output()
                            .map_err(|e| FailedToCloseSocket::with_debug(*pid, port, &e))?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Returns the IP address the hostname resolves to once it becomes available.
pub async fn wait_until_socket_open(
    pr: &Printer,
    hostname: &str,
    port: u16,
) -> Result<String, CliError> {
    pr.info(&format!(
        "Waiting for '{}:{}' to become available...",
        hostname, port
    ));

    let timeout_duration = Duration::from_secs(10 * 60); // 10 minutes
    let start_time = Instant::now();

    while start_time.elapsed() < timeout_duration {
        if let Some(ip) = socket_status(pr, hostname, port).await? {
            return Ok(ip);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    Err(SocketWaitTimeout::new(timeout_duration.as_secs()))
}

/// Returns the IP address the hostname resolves to if the socket is open.
pub async fn socket_status(
    pr: &Printer,
    hostname: &str,
    port: u16,
) -> Result<Option<String>, CliError> {
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
                return Ok(None);
            }
        }
    };

    let address_with_port = format!("{}:{}", ip_addr, port);
    let socket_addr: SocketAddr = address_with_port
        .parse()
        .map_err(|e| InvalidSocketAddress::with_debug(&address_with_port, &e))?;
    Ok(
        if TcpStream::connect_timeout(&socket_addr, Duration::from_secs(1)).is_ok() {
            Some(ip_addr)
        } else {
            None
        },
    )
}

fn is_ip_address(address: &str) -> bool {
    address.parse::<std::net::IpAddr>().is_ok()
}
