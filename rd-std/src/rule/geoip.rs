use std::io::Read;
use std::net::IpAddr;

use super::config::GeoIpMatcher;
use super::matcher::{Matcher, MaybeAsync};
use flate2::read::GzDecoder;
use maxminddb::geoip2;
use once_cell::sync::OnceCell;
use rd_interface::Address;
use tar::Archive;

// Update this when blob is updated
static GEOIP_TAR_GZ: &[u8] = include_bytes!("../../../blob/GeoLite2-Country_20210622.tar.gz");
static MMDB_PATH: &str = "GeoLite2-Country_20210622/GeoLite2-Country.mmdb";
static GEOIP_DB: OnceCell<maxminddb::Reader<Vec<u8>>> = OnceCell::new();

pub fn get_reader() -> &'static maxminddb::Reader<Vec<u8>> {
    // TODO: don't use expect
    GEOIP_DB.get_or_init(|| {
        let tar = GzDecoder::new(GEOIP_TAR_GZ);
        let mut archive = Archive::new(tar);
        let entries = archive.entries().expect("Failed to read tar");
        let mut mmdb = entries
            .filter_map(|i| i.ok())
            .find(|i| i.path().expect("Failed to read path").as_os_str() == MMDB_PATH)
            .expect("Failed to find mmdb in .tar.gz");
        let mut mmdb_buf = Vec::new();
        mmdb.read_to_end(&mut mmdb_buf)
            .expect("Failed to read mmdb");
        let reader =
            maxminddb::Reader::from_source(mmdb_buf).expect("Failed to read mmdb from source");
        reader
    })
}

impl GeoIpMatcher {
    fn test(&self, ip: impl Into<IpAddr>) -> bool {
        let ip = ip.into();
        let reader = get_reader();
        let result: Result<geoip2::Country, _> = reader.lookup(ip);
        match result {
            Ok(geoip2::Country {
                country:
                    Some(geoip2::model::Country {
                        iso_code: Some(country),
                        ..
                    }),
                ..
            }) => country == self.country,
            Err(e) => {
                tracing::debug!("Failed to lookup country for ip: {}, reason: {:?}", ip, e);
                false
            }
            _ => {
                // no message
                false
            }
        }
    }
}

impl Matcher for GeoIpMatcher {
    fn match_rule(&self, _ctx: &rd_interface::Context, addr: &Address) -> MaybeAsync<bool> {
        match addr {
            Address::SocketAddr(addr) => self.test(addr.ip()),
            // if it's a domain, try to parse it to SocketAddr.
            Address::Domain(_, _) => match addr.to_socket_addr() {
                Ok(addr) => self.test(addr.ip()),
                Err(_) => false,
            },
        }
        .into()
    }
}
