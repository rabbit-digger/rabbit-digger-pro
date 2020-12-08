/// A proxy server
pub struct ProxyServer<RT, Protocol> {
    protocol: Protocol,
    _rt: PhantomData<RT>,
}
impl<RT, Protocol> ProxyServer<RT, Protocol>
where
    Protocol: ProxyProtocol<RT>,
    RT: ProxyRuntime,
{
    pub fn new(config: Protocol::Config) -> Self {
        Self {
            protocol: Protocol::new(config),
            _rt: PhantomData,
        }
    }
    pub async fn serve() {}
}
