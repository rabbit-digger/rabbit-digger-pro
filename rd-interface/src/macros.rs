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
