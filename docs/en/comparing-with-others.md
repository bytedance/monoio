---
title: Comparing with Others
date: 2021-11-24 20:00:00
author: ihciah
---

# Comparing with Others

There are indeed many runtime like us. So what\'s the difference?

## Mio and Tokio
Mio and Tokio do not use io-uring. Instead, they use epoll-like mechanism.

Epoll/kqueue is a mechanism that allows a program to monitor multiple file descriptors for events. It is designed for watching which fd is ready to read or write. But you have to do read or write on your own later, which means more context switching between user space and kernel space.

Mio is a library to use epoll/kqueue/IOCP for different platforms. Tokio is built on top of mio providing async IO and cross-thread scheduling support. Tokio is suitable for most of the common applications and environment.

## Tokio-uring
How to use io-uring in tokio? Tokio-uring manage fd and provide an IO interface with ownership of buffer.

However, it still use epoll as notification of uring\'s fd, which is not necessary. But tokio-uring reuses most abilities of tokio, so it can be seen as a compromise in consideration of compatibility.

## Glommio
Glommio is a complex library to support common applications based on io-uring. For compatibility, Glommio copies the data, which is unnecessary. Also, we found part of Glommio implementation is not efficient.

# Performance
We compared the performance in multiple scenarios with Tokio and Glommio. Detailed comparison data can be seen [here](/docs/en/benchmark.md).
