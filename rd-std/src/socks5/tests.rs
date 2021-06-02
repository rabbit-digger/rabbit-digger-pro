use super::*;
use crate::builtin::local::{LocalConfig, LocalNet};
use crate::tests::{assert_echo, get_registry, spawn_echo_server};
use rd_interface::{IServer, IntoDyn};

#[test]
fn test_socks5_smoke() {
    let mut registry = get_registry();
    super::init(&mut registry).unwrap();
}

#[tokio::test]
async fn test_socks5_server_client() {
    let local = LocalNet::new(LocalConfig::default()).into_dyn();
    spawn_echo_server(&local, "127.0.0.1:26666").await;

    let server = server::Socks5::new(local.clone(), local.clone(), "127.0.0.1:16666".to_string());
    tokio::spawn(async move { server.start().await });

    let client = client::Socks5Client::new(local, "127.0.0.1".to_string(), 16666).into_dyn();

    assert_echo(&client, "127.0.0.1:26666").await;
}
