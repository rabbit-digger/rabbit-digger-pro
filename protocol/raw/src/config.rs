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

#[rd_config]
pub enum DeviceConfig {
    Named(String),
    Tun(TunNetConfig),
    Tap(TapNetConfig),
}

#[rd_config]
pub struct RawNetConfig {
    pub device: DeviceConfig,

    /// ipcidr
    pub server_addr: String,
    pub ethernet_addr: Option<String>,
    pub mtu: usize,
}

#[rd_config]
#[derive(Clone, Copy)]
pub enum Layer {
    L2,
    L3,
}

impl Default for Layer {
    fn default() -> Self {
        Layer::L2
    }
}
