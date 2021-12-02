---
title: 配置指南
date: 2021-11-26 14:00:00
author: ihciah
---

# 配置指南

本节将介绍 Monoio 内部的可配置选项和一些默认行为。

## 运行时配置
目前版本下，在运行时你可以改动的主要有 2 个配置：
1. entries

    entries 指 io-uring 的 ring 大小，默认是 `1024`，你可以在创建 runtime 时指定该值。注意，为了保证性能，当设定小于 256 时会设置为 256。当你的 QPS 较高时，设置较大的 entries 可以增大 ring 的大小，减少 submit 次数，这样会显著降低 syscall 占用，但也会带来一定内存占用，请合理设置。

    这个 entries 也会影响 inflight op 缓存的初始大小，默认为 `10 * entries`，目前不提供自定义接口。同样，为了避免这个缓存频繁扩容，请合理设置 entries。

    创建 runtime 时指定：
    ```rust
    RuntimeBuilder::new().with_entries(32768).build()
    ```
    通过宏指定：
    ```rust
    #[monoio::main(entries = 32768)]
    async main() {
        // ...
    }
    ```

2. enable_timer

    enable_timer 指是否启用定时器，默认不开启。如果你有异步计时需求，需要开启该功能，否则会 panic。

    哪些属于异步计时需求？即需要用到本 crate 的 time 包的时候，如异步 sleep 或者 tick 等。简单地调用标准库获取当前时间并不包括在内。

    创建 runtime 时指定：
    ```rust
    RuntimeBuilder::new().enable_timer().build()
    ```
    通过宏指定：
    ```rust
    #[monoio::main(timer_enabled)]
    async main() {
        // ...
    }
    ```

## 编译期配置
在编译期也有一些 feature 会影响 runtime 行为。
1. async-cancel

    async-cancel 默认开启。开启该 feature 后会在 Future 被 Drop 时向 io-uring 推入一个 CancelOp 来试图取消对应 Op，可能有一定的性能提升。注意，即便如此我们也并不能保证这个 Op 一定被取消。所以如果你有类似 select {读，超时} 的行为，如果你后续仍需要继续读取，请务必保存这个 Future。

2. zero-copy

    zero-copy 默认不开启。开启后会在创建 socket 时对该 socket 开启 SOCK_ZEROCOPY 标记，并在 send 时额外添加 MSG_ZEROCOPY 标记。可以减少内存拷贝，但在我们的测试中并不稳定。如果你要开启这个 feature，请确认在压力测试下系统表现正常。

3. macros

    macros 默认开启。开启该 feature 后你可以使用宏来替代 `RuntimeBuilder` 的构造函数，如 `#[monoio::main]`。

4. sync

    sync 默认不开启。这个 feature 允许你在不同线程的 Runtime 间共享 Future。如最常见的需求是，创建一个跨线程的 channel，并通过它实现线程间的通信。作为一个 thread per core 模型的 runtime，不建议在热路径上大量使用这种方式。

5. utils

    utils 默认开启。目前只有一个工具，可以允许你设置线程和 cpu 的 affinity。

6. debug

    debug 默认不开启。开启后会在运行时打印一些调试信息。仅供 Runtime 开发时调试用，不建议在生产环境开启。
