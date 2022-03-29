use super::*;
use rd_interface::{Context, INet, IntoAddress};
use rd_interface::{IServer, IntoDyn};
use rd_std::tests::{
    assert_echo, assert_echo_udp, spawn_echo_server, spawn_echo_server_udp, TestNet,
};
use std::time::Duration;
use tokio::{
    task::yield_now,
    time::{sleep, timeout},
};

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
    let client = RpcNet::new(local.clone(), "127.0.0.1:16666".into_address().unwrap()).into_dyn();
    tokio::spawn(async move { server.start().await });

    sleep(Duration::from_millis(10)).await;

    assert_echo(&client, "127.0.0.1:26666").await;
    assert_echo_udp(&client, "127.0.0.1:26666").await;

    // reverse
    spawn_echo_server(&client, "127.0.0.1:36666").await;
    spawn_echo_server_udp(&client, "127.0.0.1:36666").await;
    sleep(Duration::from_millis(10)).await;
    assert_echo(&local, "127.0.0.1:36666").await;
    assert_echo_udp(&local, "127.0.0.1:36666").await;
}

#[tokio::test]
async fn test_broken_session() {
    let local = TestNet::new().into_dyn();
    let bind_addr = "127.0.0.1:12345".into_address().unwrap();

    let server = RpcServer::new(
        local.clone(),
        local.clone(),
        "127.0.0.1:16666".into_address().unwrap(),
    );
    let client = RpcNet::new(local.clone(), "127.0.0.1:16666".into_address().unwrap());
    tokio::spawn(async move { server.start().await });

    yield_now().await;

    let listener = client
        .tcp_bind(&mut Context::new(), &bind_addr)
        .await
        .unwrap();

    assert!(local
        .tcp_bind(&mut Context::new(), &bind_addr)
        .await
        .is_err());

    let mut accept_fut = listener.accept();

    // timeout
    assert!(timeout(Duration::from_millis(10), &mut accept_fut)
        .await
        .is_err());

    client.get_sess().await.unwrap().close().await.unwrap();

    // shouldn't timeout
    let result = timeout(Duration::from_millis(10), accept_fut)
        .await
        .unwrap();
    assert!(result.is_err());

    yield_now().await;

    assert!(client
        .tcp_bind(&mut Context::new(), &bind_addr)
        .await
        .is_err());

    local
        .tcp_bind(&mut Context::new(), &bind_addr)
        .await
        .unwrap();
}
