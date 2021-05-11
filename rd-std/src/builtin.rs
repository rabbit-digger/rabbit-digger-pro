pub mod alias;
pub mod combine;
pub mod forward;
pub mod local;

pub fn init(r: &mut Registry) -> Result<()> {
    r.add_net::<alias::AliasNet>();
    r.add_net::<combine::CombineNet>();
    r.add_net::<local::LocalNet>();

    r.add_server::<forward::ForwardNet>();

    Ok(())
}
