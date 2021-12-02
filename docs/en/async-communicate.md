---
title: How to communicate within a thread or between threads
date: 2021-11-24 20:00:00
author: ihciah
---

# How to communicate within a thread or between threads

Cross-thread or cross-task communication is a very common operation. For example, if you want to do some statistics, the best performance method is to make statistics and aggregation within each thread, and finally aggregate each thread.

## Same thread asynchronous communication
Asynchronous communication with the same thread can use this crate we implemented: [local-sync](https://crates.io/crates/local-sync). It provides the implementation of mpsc (bounded/unbounded), once cell, oneshot and semaphore. Since it has no cross-thread Sync + Send implementation, its internal data structure does not require synchronization primitives such as Atomic or Mutex, which is more efficient.

## Cross-thread communication
By default, we do not support cross-thread asynchronous communication (means you can `await` tasks created by  other threads), but you can still use Atomic, etc. to achieve cross-thread communication.

To enable cross-thread asynchronous communication support, you need to enable feature `sync`. We do not provide such an implementation, you can use any implementation, such as [async-channel](https://crates.io/crates/async-channel), or use the one provided by Tokio (you need to introduce tokio dependency, you can just enable `sync` feature, it is not provided as a separate crate).
