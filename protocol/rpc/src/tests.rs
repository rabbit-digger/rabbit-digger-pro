use super::*;
use rd_interface::IntoAddress;
use rd_interface::{IServer, IntoDyn};
use rd_std::tests::{
    assert_echo, assert_echo_udp, spawn_echo_server, spawn_echo_server_udp, TestNet,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_rpc_server_client() {
    let local = TestNet::new().into_dyn();
    spawn_echo_server(&local, "127.0.0.1:26666").await;
    spawn_echo_server_udp(&local, "127.0.0.1:26666").await;

    let server = RpcServer::new(
        local.clone(),
        local.clone(),
        "127.0.0.1:16666".into_address().unwrap(),
    );
    tokio::spawn(async move { server.start().await });

    sleep(Duration::from_secs(1)).await;

    let client = RpcNet::new(local, "127.0.0.1:16666".into_address().unwrap()).into_dyn();

    assert_echo(&client, "127.0.0.1:26666").await;
    assert_echo_udp(&client, "127.0.0.1:26666").await;
}
