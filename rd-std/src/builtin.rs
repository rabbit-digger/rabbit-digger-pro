use rd_interface::{Registry, Result};

pub mod alias;
pub mod combine;
pub mod forward;
pub mod local;
pub mod noop;
pub mod resolve;

pub fn init(registry: &mut Registry) -> Result<()> {
    registry.add_net::<alias::AliasNet>();
    registry.add_net::<combine::CombineNet>();
    registry.add_net::<local::LocalNet>();
    registry.add_net::<noop::NoopNet>();
    registry.add_net::<resolve::ResolveNet>();

    registry.add_server::<forward::ForwardServer>();

    Ok(())
}
