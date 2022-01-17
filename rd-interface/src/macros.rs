#[macro_export]
macro_rules! impl_async_read_write {
    ($s:ident, $f:tt) => {
        rd_interface::impl_async_read!($s, $f);
        rd_interface::impl_async_write!($s, $f);
    };
}

#[macro_export]
macro_rules! impl_async_read {
    ($s:ident, $f:tt) => {
        impl ::rd_interface::AsyncRead for $s {
            fn poll_read(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
                buf: &mut ::rd_interface::ReadBuf,
            ) -> ::std::task::Poll<::std::io::Result<()>> {
                ::rd_interface::AsyncRead::poll_read(::std::pin::Pin::new(&mut self.$f), cx, buf)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_async_write {
    ($s:ident, $f:tt) => {
        impl ::rd_interface::AsyncWrite for $s {
            fn poll_write(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
                buf: &[u8],
            ) -> ::std::task::Poll<::std::io::Result<usize>> {
                ::rd_interface::AsyncWrite::poll_write(::std::pin::Pin::new(&mut self.$f), cx, buf)
            }

            fn poll_flush(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<::std::io::Result<()>> {
                ::rd_interface::AsyncWrite::poll_flush(::std::pin::Pin::new(&mut self.$f), cx)
            }

            fn poll_shutdown(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<::std::io::Result<()>> {
                ::rd_interface::AsyncWrite::poll_shutdown(::std::pin::Pin::new(&mut self.$f), cx)
            }

            fn poll_write_vectored(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
                bufs: &[::std::io::IoSlice<'_>],
            ) -> ::std::task::Poll<::std::io::Result<usize>> {
                ::rd_interface::AsyncWrite::poll_write_vectored(
                    ::std::pin::Pin::new(&mut self.$f),
                    cx,
                    bufs,
                )
            }

            fn is_write_vectored(&self) -> bool {
                ::rd_interface::AsyncWrite::is_write_vectored(&self.$f)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_stream_sink {
    ($s:ident, $f:tt) => {
        rd_interface::impl_stream!($s, $f);
        rd_interface::impl_sink!($s, $f);
    };
}

#[macro_export]
macro_rules! impl_stream {
    ($s:ident, $f:tt) => {
        impl ::rd_interface::Stream for $s {
            type Item = ::std::io::Result<(::rd_interface::Bytes, ::std::net::SocketAddr)>;

            fn poll_next(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<Option<Self::Item>> {
                ::rd_interface::Stream::poll_next(::std::pin::Pin::new(&mut self.$f), cx)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_sink {
    ($s:ident, $f:tt) => {
        impl ::rd_interface::Sink<(::rd_interface::Bytes, ::rd_interface::Address)> for $s {
            type Error = ::std::io::Error;

            fn poll_ready(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut ::std::task::Context<'_>,
            ) -> ::std::task::Poll<::std::result::Result<(), Self::Error>> {
                ::rd_interface::Sink::poll_ready(::std::pin::Pin::new(&mut self.$f), cx)
            }

            fn start_send(
                mut self: ::std::pin::Pin<&mut Self>,
                item: (::rd_interface::Bytes, ::rd_interface::Address),
            ) -> ::std::result::Result<(), Self::Error> {
                ::rd_interface::Sink::start_send(::std::pin::Pin::new(&mut self.$f), item)
            }

            fn poll_flush(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> ::std::task::Poll<::std::result::Result<(), Self::Error>> {
                ::rd_interface::Sink::poll_flush(::std::pin::Pin::new(&mut self.$f), cx)
            }

            fn poll_close(
                mut self: ::std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> ::std::task::Poll<::std::result::Result<(), Self::Error>> {
                ::rd_interface::Sink::poll_close(::std::pin::Pin::new(&mut self.$f), cx)
            }
        }
    };
}
