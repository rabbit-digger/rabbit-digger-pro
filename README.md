# rabbit-digger-pro

All-in-one proxy written in Rust.

See also: [rabbit-digger/rabbit-digger](https://github.com/rabbit-digger/rabbit-digger)

## Features

* Hot reloading: Apply changes without restart the program.
* Flexible configuration: proxies can be nested at will, supporting TCP and UDP.
* JSON Schema generation: no documentation needed, write configuration directly from code completion.

### Supported Protocol

* Shadowsocks
* Trojan
* HTTP
* Socks5
* obfs(http_simple)

### Supported Server Protocol

* Socks5
* HTTP
* http+socks5 on the same port
* Shadowsocks

## Credits

* [shadowsocks-rust](https://github1s.com/shadowsocks/shadowsocks-rust)
* [smoltcp](https://github.com/smoltcp-rs/smoltcp)
