---
title: Compatible with Tokio ecology
date: 2021-12-17 18:00:00
author: ihciah
---

# Compatible with Tokio ecology
A large number of existing Rust components are compatible with Tokio, and they directly rely on Tokio's `AsyncRead` and `AsyncWrite` interfaces.

In Monoio, since the bottom layer is an asynchronous system call, we chose a similar tokio-uring approach: providing an IO interface that transfers the ownership of the buffer. But at this stage, obviously there are not many available libraries that can work, so we need to quickly support functions with a certain performance sacrifice.

## monoio-compat
`monoio-compat` is a compatibility layer that implements Tokio's `AsyncRead` and `AsyncWrite` based on the buffer ownership interface.

### How it works
For `poll_read`, the remaining capacity of the slice passed by the user will be read first, and then the buffer held by the user will be limited to this capacity and a future will be generated. After that, every time the user `poll_read` again, it will be forwarded to the `poll` method of this future. When returning to `Ready`, the contents of the buffer are copied to the slice passed by the user.

For `poll_write`, the content of the slice passed by the user is copied to its own buffer first, then a future is generated and stored, and Ready is returned immediately. After that, every time the user `poll_write` again, it will first wait for the last content to be sent, then copy the data and return to Ready immediately. It behaves like BufWriter, and may causing errors to be delayed to be perceived.

At the cost, using this compatibility layer will cost you an extra buffer copy.

For `poll_write`, the content of the slice passed by the user is first copied to its own buffer, and then a future is generated and stored. After that, every time the user `poll_write` again, it will be forwarded to the `poll` method of this future. Return the result to the user when returning `Ready`.

In other words, using this compatibility layer will cost you an extra buffer copy overhead.

### Usage restrictions
For write operations, users need to manually call poll_flush or poll_shutdown of AsyncWrite at the end (or the corresponding flush or shutdown methods of AsyncWriteExt), otherwise the data may not be submitted to the kernel (continuous writes do not require manual flushing).

## Poll-oriented interface and asynchronous system call
There are two ways to express asynchrony in Rust, one is based on `poll` and the other is based on `async`. `poll` is synchronous, semantically expressing an immediate attempt; while `async` is essentially the syntactic sugar of poll, it will swallow the future and generate a state machine, which is executed in a loop during await.

In Tokio, there are methods similar to `poll_read` and `poll_write`, both of which express the semantics of synchronous attempts.

When it returns to `Pending`, it means that IO is not ready (and the waker of cx is registered for notification), and when it returns to `Ready` it means that IO has been completed. It is easy to implement these two interfaces based on synchronous system calls. Directly make the corresponding system calls and determine the return result. If the IO is not ready, it will be suspended to Reactor.

However, these two interfaces are difficult to implement under asynchronous system calls. If we have pushed an Op into the io_uring SQ, the state of this syscall is uncertain before the corresponding CQE is consumed. We have no way to provide clear completed or unfinished semantics. In `monoio-compat`, we provide a poll-like interface through some hacks, so the lack of capabilities has led to our use restrictions. Under asynchronous system calls, it is more appropriate to pass the ownership of the buffer and cooperate with `async+await`.

At present, the Rust standard library does not have a interface for asynchronous system calls, and there is no related component ecology. We are working hard to solve this problem.