/// Custom listener options
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ListenerOpts {
    /// Whether to enable reuse_port.
    pub reuse_port: bool,
    /// Whether to enable reuse_addr.
    pub reuse_addr: bool,
    /// Backlog size.
    pub backlog: i32,
    /// Send buffer size or None to use default.
    pub send_buf_size: Option<usize>,
    /// Recv buffer size or None to use default.
    pub recv_buf_size: Option<usize>,
    /// TCP fast open.
    pub tcp_fast_open: bool,
}

impl Default for ListenerOpts {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ListenerOpts {
    /// Create a default ListenerOpts.
    #[inline]
    pub const fn new() -> Self {
        Self {
            reuse_port: true,
            reuse_addr: true,
            backlog: 1024,
            send_buf_size: None,
            recv_buf_size: None,
            tcp_fast_open: false,
        }
    }

    /// Enable SO_REUSEPORT
    #[must_use]
    #[inline]
    pub fn reuse_port(mut self, reuse_port: bool) -> Self {
        self.reuse_port = reuse_port;
        self
    }

    /// Enable SO_REUSEADDR
    #[must_use]
    #[inline]
    pub fn reuse_addr(mut self, reuse_addr: bool) -> Self {
        self.reuse_addr = reuse_addr;
        self
    }

    /// Specify backlog
    #[must_use]
    #[inline]
    pub fn backlog(mut self, backlog: i32) -> Self {
        self.backlog = backlog;
        self
    }

    /// Specify SO_SNDBUF
    #[must_use]
    #[inline]
    pub fn send_buf_size(mut self, send_buf_size: usize) -> Self {
        self.send_buf_size = Some(send_buf_size);
        self
    }

    /// Specify SO_RCVBUF
    #[must_use]
    #[inline]
    pub fn recv_buf_size(mut self, recv_buf_size: usize) -> Self {
        self.recv_buf_size = Some(recv_buf_size);
        self
    }

    /// Specify FastOpen.
    /// Note: if it is enabled, the connection will be
    /// established on first peer data sent, which means
    /// data cannot be sent immediately after connection
    /// accepted if client does not send something.
    #[must_use]
    #[inline]
    pub fn tcp_fast_open(mut self, fast_open: bool) -> Self {
        self.tcp_fast_open = fast_open;
        self
    }
}
