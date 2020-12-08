//! Defines traits used in APiR.
pub use runtime::*;

mod runtime;

pub trait ProxyProtocol<RT: ProxyRuntime> {
    type Context;
    type Config;
    fn new(config: Self::Config) -> Self;
}
