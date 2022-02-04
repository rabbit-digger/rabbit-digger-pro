use rd_interface::{
    config::{Config, NetRef},
    prelude::*,
};

#[rd_config]
#[serde(rename_all = "lowercase")]
pub enum TunTap {
    Tap,
    Tun,
}

#[rd_config]
pub struct TunTapConfig {
    #[serde(rename = "type")]
    pub tuntap: TunTap,
    pub name: Option<String>,
    /// host address
    pub host_addr: String,
}

#[rd_config]
#[serde(untagged)]
pub enum MaybeString<T>
where
    T: Config,
{
    String(String),
    Other(T),
}

pub type DeviceConfig = MaybeString<TunTapConfig>;

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

#[rd_config]
pub struct RawNetConfig {
    #[serde(default)]
    pub net: NetRef,
    pub device: DeviceConfig,
    pub gateway: Option<String>,

    /// IP Cidr
    pub ip_addr: String,
    pub ethernet_addr: Option<String>,
    pub mtu: usize,

    #[serde(default)]
    pub forward: bool,
}
