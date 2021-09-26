use rd_interface::prelude::*;

#[rd_config]
pub struct TunNetConfig {
    pub dev_name: Option<String>,
    /// tun address
    pub tun_addr: String,
    /// ipcidr
    pub server_addr: String,
    pub ethernet_addr: Option<String>,
    pub mtu: usize,
}

#[rd_config]
pub struct TapNetConfig {
    pub dev_name: Option<String>,
    /// tap address
    pub tap_addr: String,
    /// ipcidr
    pub server_addr: String,
    pub ethernet_addr: Option<String>,
    pub mtu: usize,
}
