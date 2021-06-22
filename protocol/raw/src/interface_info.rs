#![allow(dead_code)]

use smoltcp::wire::EthernetAddress;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Mac address not found")]
    NotFound,
    #[error("Failed to call system")]
    FailedToCallSystem,
}

pub struct InterfaceInfo {
    pub ethernet_address: EthernetAddress,
    pub name: String,
    pub description: Option<String>,
}

impl Default for InterfaceInfo {
    fn default() -> Self {
        InterfaceInfo {
            ethernet_address: EthernetAddress::BROADCAST,
            name: "".to_string(),
            description: None,
        }
    }
}

#[cfg(unix)]
pub use unix::get_interface_info;
#[cfg(windows)]
pub use windows::get_interface_info;

#[cfg(unix)]
mod unix {
    use super::{Error, InterfaceInfo};
    use smoltcp::wire::EthernetAddress;

    impl From<nix::Error> for Error {
        fn from(_: nix::Error) -> Error {
            Error::FailedToCallSystem
        }
    }

    pub fn get_interface_info(name: &str) -> Result<InterfaceInfo, Error> {
        use nix::{ifaddrs::getifaddrs, sys::socket::SockAddr};
        let addrs = getifaddrs()?;
        for ifaddr in addrs {
            if ifaddr.interface_name != name {
                continue;
            }
            if let Some(SockAddr::Link(link)) = ifaddr.address {
                return Ok(InterfaceInfo {
                    ethernet_address: EthernetAddress(link.addr()),
                    name: name.into(),
                    description: None,
                });
            }
        }
        Err(Error::NotFound)
    }
}

#[cfg(windows)]
mod windows {
    use super::{Error, InterfaceInfo};
    use smoltcp::wire::EthernetAddress;
    use std::{mem, ptr};

    const NO_ERROR: u32 = 0;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

    fn get_guid(s: &str) -> Option<&str> {
        if let Some(pos) = s.find('{') {
            let p = pos + 1;
            if let Some(end) = s[p..].find('}') {
                return Some(&s[p..(p + end)]);
            }
        }
        return None;
    }

    fn from_u16(s: &[u16]) -> Option<String> {
        if let Some(pos) = s.iter().position(|c| *c == 0) {
            if let Ok(string) = String::from_utf16(&s[0..pos]) {
                return Some(string);
            }
        }
        return None;
    }

    pub fn get_interface_info(name: &str) -> Result<InterfaceInfo, Error> {
        if let Some(intf_guid) = get_guid(name) {
            let mut size = 0u32;
            let table: *mut MibIftable;

            let mut info = InterfaceInfo {
                name: name.to_string(),
                ..Default::default()
            };

            unsafe {
                if GetIfTable(
                    ptr::null_mut::<MibIftable>(),
                    &mut size as *mut libc::c_ulong,
                    false,
                ) == ERROR_INSUFFICIENT_BUFFER
                {
                    table = mem::transmute(libc::malloc(size as libc::size_t));
                } else {
                    return Err(Error::NotFound);
                }

                if GetIfTable(table, &mut size as *mut libc::c_ulong, false) == NO_ERROR {
                    let ptr: *const MibIfrow = (&(*table).table) as *const _;
                    let table = std::slice::from_raw_parts(ptr, (*table).dw_num_entries as usize);
                    for i in table {
                        let row = &*i;

                        if let Some(name) = from_u16(&row.wsz_name) {
                            if let Some(guid) = get_guid(&name) {
                                if guid == intf_guid {
                                    if row.dw_phys_addr_len == 6 {
                                        info.ethernet_address =
                                            EthernetAddress::from_bytes(&row.b_phys_addr[0..6]);
                                    } else {
                                        continue;
                                    }
                                    if row.dw_descr_len > 0 {
                                        if let Ok(desc) = String::from_utf8(
                                            row.b_descr[..(row.dw_descr_len - 1) as usize].to_vec(),
                                        ) {
                                            info.description = Some(desc);
                                        }
                                    }
                                    return Ok(info);
                                }
                            }
                        }
                    }
                }
                libc::free(mem::transmute(table));
            }
        }
        Err(Error::NotFound)
    }

    pub const MAX_INTERFACE_NAME_LEN: usize = 256;
    pub const MAXLEN_PHYSADDR: usize = 8;
    pub const MAXLEN_IFDESCR: usize = 256;

    #[repr(C)]
    pub(crate) struct MibIfrow {
        pub wsz_name: [u16; MAX_INTERFACE_NAME_LEN],
        pub dw_index: u32,
        pub dw_type: u32,
        pub dw_mtu: u32,
        pub dw_speed: u32,
        pub dw_phys_addr_len: u32,
        pub b_phys_addr: [u8; MAXLEN_PHYSADDR],
        _padding1: [u8; 15 * 4],
        pub dw_descr_len: u32,
        pub b_descr: [u8; MAXLEN_IFDESCR],
    }

    #[repr(C)]
    pub(crate) struct MibIftable {
        pub dw_num_entries: u32,
        pub table: MibIfrow,
    }

    #[link(name = "iphlpapi")]
    #[allow(non_snake_case)]
    extern "system" {
        pub(crate) fn GetIfTable(
            table: *mut MibIftable,
            size: *mut libc::c_ulong,
            order: bool,
        ) -> u32;
    }
}
