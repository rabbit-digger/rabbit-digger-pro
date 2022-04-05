use async_stream::try_stream;
use futures::Stream;
use std::io;
use tokio::signal::ctrl_c;

pub fn exit_stream() -> impl Stream<Item = io::Result<usize>> {
    let mut times = 0;
    try_stream! {
        loop {
            ctrl_c().await?;
            times += 1;
            yield times;
        }
    }
}
