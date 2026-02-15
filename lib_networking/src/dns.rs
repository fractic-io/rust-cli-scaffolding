use std::{
    iter,
    net::{IpAddr, SocketAddr},
    str::FromStr as _,
};

use hickory_client::{
    client::{AsyncClient, ClientHandle as _},
    proto::iocompat::AsyncIoTokioAsStd,
    rr::{DNSClass, Name, RData, RecordType},
    tcp::TcpClientStream,
};
use lib_core::{define_cli_error, CliError};
use tokio::net::TcpStream;

define_cli_error!(
    DnsConnectionError,
    "Failed to establish a connection to the DNS server: {details}.",
    { details: &str }
);
define_cli_error!(
    InvalidDnsRequest,
    "Failed to send DNS request: {details}.",
    { details: &str }
);
define_cli_error!(
    DnsRecordNotFound,
    "No {rtype} record found for the given address.",
    { rtype: &str }
);

const FALLBACK_NAME_SERVER: &str = "8.8.8.8"; // Google
const DNS_PORT: u16 = 53;

// Public functions.
// ----------------------------------------------------------------------------

pub async fn dns_query_a_record(address: &str) -> Result<String, CliError> {
    if address == "localhost" {
        return Ok("127.0.0.1".to_string());
    }

    query(address, RecordType::A).await
}

pub async fn dns_query_cname_record(address: &str) -> Result<String, CliError> {
    query(address, RecordType::CNAME).await
}

// Internal.
// ----------------------------------------------------------------------------

async fn query(address: &str, record_type: RecordType) -> Result<String, CliError> {
    let name = Name::from_str(address)
        .map_err(|e| InvalidDnsRequest::with_debug("could not parse address", &e))?;
    let mut last_error: Option<CliError> = None;

    for nameserver in configured_nameservers().await {
        let socket_address = match to_socket_address(&nameserver) {
            Ok(address) => address,
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        };

        let (stream, sender) = TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::new(socket_address);

        let (mut client, bg) = match AsyncClient::new(stream, sender, None).await {
            Ok(result) => result,
            Err(e) => {
                last_error = Some(DnsConnectionError::with_debug(
                    "could not establish connection",
                    &e,
                ));
                continue;
            }
        };

        tokio::spawn(bg);

        let response = match client.query(name.clone(), DNSClass::IN, record_type).await {
            Ok(response) => response,
            Err(e) => {
                last_error = Some(DnsConnectionError::with_debug("could not send query", &e));
                continue;
            }
        };

        let resolved_value = match record_type {
            RecordType::A => response.answers().iter().find_map(|record| {
                if let Some(RData::A(ip)) = record.data() {
                    Some(ip.to_string())
                } else {
                    None
                }
            }),
            RecordType::CNAME => response.answers().iter().find_map(|record| {
                if let Some(RData::CNAME(cname)) = record.data() {
                    Some(cname.to_string())
                } else {
                    None
                }
            }),
            _ => None,
        };

        if let Some(value) = resolved_value {
            return Ok(value);
        }

        let not_found_error = DnsRecordNotFound::new(match record_type {
            RecordType::A => "A",
            RecordType::CNAME => "CNAME",
            _ => "UNKNOWN",
        });

        // An authoritative negative response (e.g. NXDOMAIN/NODATA) is final.
        if response.header().authoritative() {
            return Err(not_found_error);
        }

        last_error = Some(not_found_error);
    }

    Err(last_error.unwrap_or_else(|| DnsConnectionError::new("could not query DNS records")))
}

// Helpers.
// ----------------------------------------------------------------------------

async fn configured_nameservers() -> Vec<String> {
    tokio::fs::read_to_string("/etc/resolv.conf")
        .await
        .ok()
        .map(|content| {
            content
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        return None;
                    }
                    line.strip_prefix("nameserver")
                        .and_then(|rest| rest.split_whitespace().next())
                        .map(|value| value.to_string())
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
        .into_iter()
        .chain(iter::once(FALLBACK_NAME_SERVER.to_string()))
        .collect()
}

fn to_socket_address(nameserver: &str) -> Result<SocketAddr, CliError> {
    if let Ok(ip) = nameserver.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, DNS_PORT));
    }

    nameserver
        .parse::<SocketAddr>()
        .map_err(|e| DnsConnectionError::with_debug("could not parse name server address", &e))
}
