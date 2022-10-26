/// Custom listener config
#[derive(Debug, Clone, Copy)]
pub struct ListenerConfig {
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
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            reuse_port: true,
            reuse_addr: true,
            backlog: 1024,
            send_buf_size: None,
            recv_buf_size: None,
        }
    }
}

impl ListenerConfig {
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
}
