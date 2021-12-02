---
title: Configuration Guide
date: 2021-11-26 14:00:00
author: ihciah
---

# Configuration Guide

This section describes the configurable options and some of the default behavior inside Monoio.

## Runtime Configuration
With the current version, there are 2 main configurations that you can change at runtime.
1. entries

    entries refers to the ring size of the io-uring, the default is `1024`, you can specify this value when creating the runtime. Note that for performance, the setting is set to 256 when it is less than 256. When your QPS is high, setting larger entries can increase the ring size and reduce the number of submits, which will significantly reduce the syscall usage, but also bring some memory usage, please set it reasonably.

    The entries also affects the initial size of the inflight op cache, which defaults to `10 * entries` and currently does not provide a custom interface. Again, to avoid frequent expansion of this cache, please set entries wisely.

    When creating a runtime, specify.
    ```rust
    RuntimeBuilder::new().with_entries(32768).build()
    ```
    Specified by macro.
    ```rust
    #[monoio::main(entries = 32768)]
    async main() {
        // ...
    }
    ```

2. enable_timer

    enable_timer means whether to enable the timer or not, the default is not enabled. If you have asynchronous timing requirements, you need to enable this feature, otherwise it will panic.

    What are the asynchronous timing requirements? That is, when you need to use the time module of this crate, such as asynchronous sleep or tick, etc. Simply calling the standard library to get the current time is not included.

    When creating a runtime, specify:
    ```rust
    RuntimeBuilder::new().enable_timer().build()
    ```
    Specified via macro:
    ```rust
    #[monoio::main(timer_enabled)]
    async main() {
        // ...
    }
    ```

## Compile-time configuration
There are also some features that affect runtime behavior during compile time.
1. async-cancel

    async-cancel is enabled by default. Turning this feature on pushes a CancelOp into the io-uring to try to cancel the corresponding Op when the Future is dropped, which may have some performance improvements. Note that even then we can't guarantee that the Op will be cancelled. So if you have something like select {read, timeout}, be sure to save the Future if you still need to read it later.

2. zero-copy

    zero-copy is not enabled by default. When enabled, it turns on the SOCK_ZEROCOPY flag for the socket when it is created, and adds an additional MSG_ZEROCOPY flag when it is sent. This reduces memory copies, but is not stable in our tests. If you want to enable this feature, please make sure that the system behaves properly under stress tests.

3. macros

    macros is enabled by default. With this feature on you can use macros instead of the `RuntimeBuilder` constructor, such as `#[monoio::main]`.

4. sync

    sync is not enabled by default. This feature allows you to share Future between different threads of Runtime, e.g. the most common requirement is to create a cross-thread channel and use it to communicate between threads. As a thread per core model runtime, it is not recommended to use this approach heavily on the hot path.

5. utils

    utils is turned on by default. Currently there is only one utility that allows you to set the affinity of threads and cpu.

6. debug

    debug is not enabled by default. It will print some debugging information at runtime when enabled. It is only for debugging during Runtime development and is not recommended to be enabled in production environment.
