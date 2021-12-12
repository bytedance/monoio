---
title: 与其他 Runtime 的对比
date: 2021-11-24 20:00:00
author: ihciah
---

# 与其他 Runtime 的对比

类似的 Runtime 还有很多，本文主要介绍区别和设计上的取舍。

## Mio 和 Tokio
Mio 和 Tokio 不使用 io-uring，它们使用 epoll(在 linux 上)。以 linux 平台为例，epoll 一种 IO 通知机制，利用它可以监控并等待 fd ready。但是，你仍需要在 fd ready 后调用 read/write，这也是会有额外的用户态和内核态的切换开销。

Mio 是 Tokio 的底层 IO 库，它屏蔽了 epoll/kqueue/IOCP 在不同平台上的差异。

Tokio 内部除了利用 Mio 做 IO 多路复用以外，还做了跨线程的任务调度（实现和 golang 很像）。对于绝大多数通用场景来说，Tokio 是很合适的；同时它也提供了非常好的兼容性。

## Tokio-uring
在 Tokio 内能使用 io-uring 吗？可以试试 Tokio-uring。Tokio-uring 提供了基于 io-uring 的 IO 接口，需要持有 buffer 所有权操作。

但是它并不是彻底的 io-uring，为了复用 Tokio 的底层能力，它还是将 uring fd 构建在 epoll 之上，不考虑兼容的话这显然是不必要的。

## Glommio
Glommio 是一个复杂的基于 io-uring 的库。内部有多个 ring，设计上尽可能兼容多种场景，而这种多 ring 的设计这也带来了一些额外的性能开销。在 IO 操作接口上，它提供了类似 Tokio 的接口，由于 kernel 读写数据是异步的，所以它内部做了一次额外的数据拷贝，在高性能场景下也带来了一些不期望的开销。

## Monoio
Monoio 设计上以性能为最优先。它也不是银弹，如果你的业务场景中任务不太均匀，那么很可能会导致不同核心的利用率出现差别。对于合适的场景，如常规的代理场景，Monoio 的性能是要好于其他 Runtime 的。详细的数据可以 Monoio Benchmark。

# 性能对比
我们与 Tokio 和 Glommio 对比了多个场景下的性能，详细的对比数据可以看 [对比数据](/docs/zh/benchmark.md)。虽然我们在测试数据上要优于其他 Runtime，但这并不意味着其他 Runtime 是个“坏”的设计，只是面向目标场景不同带来的取舍上的差别。
