use std::str::FromStr as _;

use hickory_client::{
    client::{Client, SyncClient},
    rr::{DNSClass, Name, RData, RecordType},
    udp::UdpClientConnection,
};
use lib_core::{define_cli_error, CliError};

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

pub fn dns_query_a_record(address: &str) -> Result<String, CliError> {
    let conn =
        UdpClientConnection::new(NAME_SERVER.parse().map_err(|e| {
            DnsConnectionError::with_debug("could not parse name server address", &e)
        })?)
        .map_err(|e| DnsConnectionError::with_debug("could not establish connection", &e))?;
    let client = SyncClient::new(conn);
    client
        .query(
            &Name::from_str(address)
                .map_err(|e| InvalidDnsRequest::with_debug("could not parse address", &e))?,
            DNSClass::IN,
            RecordType::A,
        )
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

pub fn dns_query_cname_record(address: &str) -> Result<String, CliError> {
    let conn =
        UdpClientConnection::new(NAME_SERVER.parse().map_err(|e| {
            DnsConnectionError::with_debug("could not parse name server address", &e)
        })?)
        .map_err(|e| DnsConnectionError::with_debug("could not establish connection", &e))?;
    let client = SyncClient::new(conn);
    client
        .query(
            &Name::from_str(address)
                .map_err(|e| InvalidDnsRequest::with_debug("could not parse address", &e))?,
            DNSClass::IN,
            RecordType::CNAME,
        )
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
