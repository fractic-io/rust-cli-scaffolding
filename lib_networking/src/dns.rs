use std::str::FromStr as _;

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

const NAME_SERVER: &'static str = "8.8.8.8:53";

pub async fn dns_query_a_record(address: &str) -> Result<String, CliError> {
    let (stream, sender) =
        TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::new(NAME_SERVER.parse().map_err(|e| {
            DnsConnectionError::with_debug("could not parse name server address", &e)
        })?);
    let (mut client, bg) = AsyncClient::new(stream, sender, None)
        .await
        .map_err(|e| DnsConnectionError::with_debug("could not establish connection", &e))?;
    tokio::spawn(bg);
    client
        .query(
            Name::from_str(address)
                .map_err(|e| InvalidDnsRequest::with_debug("could not parse address", &e))?,
            DNSClass::IN,
            RecordType::A,
        )
        .await
        .map_err(|e| DnsConnectionError::with_debug("could not send query", &e))
        .and_then(|response| {
            response
                .answers()
                .iter()
                .find_map(|record| {
                    if let Some(RData::A(ip)) = record.data() {
                        Some(ip.to_string())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| DnsRecordNotFound::new("A"))
        })
}

pub async fn dns_query_cname_record(address: &str) -> Result<String, CliError> {
    let (stream, sender) =
        TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::new(NAME_SERVER.parse().map_err(|e| {
            DnsConnectionError::with_debug("could not parse name server address", &e)
        })?);
    let (mut client, bg) = AsyncClient::new(stream, sender, None)
        .await
        .map_err(|e| DnsConnectionError::with_debug("could not establish connection", &e))?;
    tokio::spawn(bg);
    client
        .query(
            Name::from_str(address)
                .map_err(|e| InvalidDnsRequest::with_debug("could not parse address", &e))?,
            DNSClass::IN,
            RecordType::CNAME,
        )
        .await
        .map_err(|e| DnsConnectionError::with_debug("could not send query", &e))
        .and_then(|response| {
            response
                .answers()
                .iter()
                .find_map(|record| {
                    if let Some(RData::CNAME(cname)) = record.data() {
                        Some(cname.to_string())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| DnsRecordNotFound::new("CNAME"))
        })
}
