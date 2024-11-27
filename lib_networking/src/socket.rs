use std::process::Command;

use lib_core::{define_cli_error, CliError, Printer};
use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};

define_cli_error!(
    GetSocketInfoError,
    "Failed to get info on currently open sockets."
);
define_cli_error!(
    FailedToCloseSocket,
    "Failed to kill PID {pid} to free port {port}.",
    { pid: u32, port: u16 }
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
