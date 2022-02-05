use rd_interface::{
    config::{Config, NetRef},
    prelude::*,
};
use tokio_smoltcp::smoltcp::phy::Medium;

#[rd_config]
#[serde(rename_all = "lowercase")]
#[derive(Copy, Clone)]
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

impl From<Layer> for tun_crate::Layer {
    fn from(l: Layer) -> Self {
        match l {
            Layer::L2 => tun_crate::Layer::L2,
            Layer::L3 => tun_crate::Layer::L3,
        }
    }
}

impl From<TunTap> for Layer {
    fn from(t: TunTap) -> Self {
        match t {
            TunTap::Tap => Layer::L2,
            TunTap::Tun => Layer::L3,
        }
    }
}

impl From<Medium> for Layer {
    fn from(m: Medium) -> Self {
        match m {
            Medium::Ethernet => Layer::L2,
            Medium::Ip => Layer::L3,
            _ => panic!("unsupported medium"),
        }
    }
}

impl From<Layer> for Medium {
    fn from(l: Layer) -> Self {
        match l {
            Layer::L2 => Medium::Ethernet,
            Layer::L3 => Medium::Ip,
        }
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
