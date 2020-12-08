pub struct ProxyClient<RT, Protocol> {
    protocol: Protocol,
    _rt: PhantomData<RT>,
}

impl<RT, Protocol> ProxyClient<RT, Protocol>
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
}
