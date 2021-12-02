---
title: 如何在同线程或跨线程使用异步通信
date: 2021-11-24 20:00:00
author: ihciah
---

# 如何在同线程或跨线程使用异步通信

跨线程或跨 Task 通信是一个很常见的操作。比如你想做一些统计，那么性能最佳的办法是，每个线程内部自己统计并聚合，最后各个线程再聚合。

## 同线程异步通信
同线程异步通信可以使用我们实现的这个 crate：[local-sync](https://crates.io/crates/local-sync)。它提供了 mpsc(bounded/unbounded)、once cell、oneshot 和 semaphore 的实现。由于它没有跨线程 Sync + Send 实现，其内部数据结构不需要 Atomic 或 Mutex 等同步原语，较为高效。

## 跨线程通信
默认情况下我们不支持跨线程异步通信（指可以 `await` 其他线程创建的 Task），但你仍然可以使用如 Atomic 等实现跨线程通信的目的。

若要开启跨线程异步通信支持，需要开启 feature `sync`。我们没有提供这类实现，你可以使用任何实现，如[async-channel](https://crates.io/crates/async-channel)，或者使用 Tokio 提供的（需要引入 tokio 依赖，可以只开启 `sync` feature，它没有以单独的 crate 提供）。
