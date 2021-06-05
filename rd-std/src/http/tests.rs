use super::*;
use crate::builtin::local::{LocalConfig, LocalNet};
use crate::tests::{assert_echo, get_registry, spawn_echo_server};
use rd_interface::{IServer, IntoDyn};
use std::time::Duration;
use tokio::time::sleep;

#[test]
fn test_http_smoke() {
    let mut registry = get_registry();
    super::init(&mut registry).unwrap();
}

#[tokio::test]
async fn test_http_server_client() {
    let local = LocalNet::new(LocalConfig::default()).into_dyn();
    spawn_echo_server(&local, "127.0.0.1:26667").await;

    let server = server::Http::new(local.clone(), local.clone(), "127.0.0.1:16667".to_string());
    tokio::spawn(async move { server.start().await });

    sleep(Duration::from_secs(1)).await;

    let client = client::HttpClient::new(local, "127.0.0.1".to_string(), 16667).into_dyn();

    assert_echo(&client, "127.0.0.1:26667").await;
}
