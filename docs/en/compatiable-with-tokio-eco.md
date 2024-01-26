---
title: Compatible with Tokio ecology
date: 2021-12-17 18:00:00
updated: 2024-01-30 15:00:00
author: ihciah
---

# Compatible with the Tokio Ecosystem
A large number of existing Rust components are compatible with Tokio and directly depend on Tokio's `AsyncRead` and `AsyncWrite` traits.

In Monoio, due to the underlying asynchronous system calls, we chose an approach similar to tokio-uring: providing IO interfaces that transfer buffer ownership. However, at this stage, there are obviously not many libraries available that work with this, so we need some means to be compatible with existing interface components.

Currently, there are 3 ways to achieve compatibility:

## tokio-compat
`tokio-compat` is a feature of monoio. When the `iouring` is disabled and the `legacy` feature is enabled, after turning on the `tokio-compat` feature, `TcpStream`/`UnixStream` will implement `tokio::io::{AsyncRead, AsyncWrite}`.

If you explicitly do not use iouring, then you can provide compatibility in this way. This form of compatibility has no overhead. If you might use iouring, then you should use the `poll-io` feature.

## poll-io
`poll-io` is a feature of monoio. After enabling this feature:
1. `tokio::io` will be re-exported to `monoio::io::poll_io`
2. `TcpStream`/`UnixStream` can be converted to and from `TcpStreamPoll`/`UnixStreamPoll`
3. `TcpStreamPoll`/`UnixStreamPoll` implements `tokio::io::{AsyncRead, AsyncWrite}`

The underlying implementation of this feature runs epoll to sense fd readiness on top of iouring and directly initiates a syscall. Although this form of compatibility cannot effectively utilize iouring for asynchronous io, its performance is similar to other epoll+syscall implementations without additional copy overhead.

## monoio-compat
`monoio-compat` is a compatibility layer that implements Tokio's `AsyncRead` and `AsyncWrite` based on an interface that transfers buffer ownership.

### How It Works
For `poll_read`, it first reads into the remaining capacity of the slice passed by the user, then restricts its own buffer to that capacity and generates a future. Afterwards, every time the user again calls `poll_read`, it will forward to the `poll` method of this future. When returning `Ready`, it copies the contents of the buffer into the user's passed slice.

For `poll_write`, it first copies the contents of the user's passed slice into its own buffer, then generates a future and stores it, and immediately returns Ready. Thereafter, every time the user again calls `poll_write`, it will first wait for the content of the last write to be fully sent before copying data and immediately returning Ready. This behavior is similar to that of a BufWriter, which can lead to delayed error detection.

The cost of using this compatibility layer is an additional buffer copy overhead.

### Usage Restrictions
For write operations, users need to manually call AsyncWrite's poll_flush or poll_shutdown (or the corresponding flush or shutdown methods in AsyncWriteExt) at the end; otherwise, data may not be submitted to the kernel (continuous writes do not require manual flushing).

## Poll-Based Interfaces and Asynchronous System Calls
There are two ways to express asynchrony in Rust: one is based on `poll`, and the other is based on `async`. `Poll` is synchronous and semantically expresses the trying immediately; while `async` is essentially syntactic sugar for poll which encapsulates the future and generates a state machine, executing this state machine in a loop when awaiting.

In Tokio, there are methods like `poll_read` and `poll_write` that both express the semantic of synchronous trying.

When `Pending` is returned, it implies that the IO is not ready (and registers a notice with the waker in cx), and when `Ready` is returned, it means the IO has completed. It is easy to implement these two interfaces based on synchronous system calls, by directly making the corresponding system call and judging the return result, and if the IO is not ready, suspend to the Reactor.

However, these two interfaces are difficult to implement under asynchronous system calls. If we have already pushed an Op into the io_uring SQ, then the status of that syscall is uncertain until we consume the corresponding CQE. We cannot provide a clear completed or not completed semantics. In `monoio-compat`, we provide a poll-like interface through some hacks, so the lack of capabilities leads to our usage restrictions. Under asynchronous system calls, transferring buffer ownership in combination with `async+await` is more appropriate.

Currently, Rust's standard library does not have a universal interface oriented towards asynchronous system calls, and neither does the related component ecosystem. We are working hard to improve this problem.