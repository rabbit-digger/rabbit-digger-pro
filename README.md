# rabbit-digger

> A picture shows a rabbit digging a wall.

Rule based async tun to proxy based on smoltcp written in Rust.

## Build using Cross

```sh
cargo install cross # run once
cross build --release --features=tracing-subscriber --target mipsel-unknown-linux-musl
```
