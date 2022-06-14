---
title: 使用 Legacy 驱动
date: 2022-05-25 21:00:00
author: ihciah
---

虽然 Monoio 的目标平台是支持 io_uring 的 Linux，但是当你对此并不可控；或者想平滑迁移；或者想在 macOS 做开发的时候，可以使用 Legacy 驱动。

Legacy 驱动目前支持 macOS 和 Linux，分别基于 kqueue 和 epoll。

## 启动配置
第一种配置方式是通过宏：
```rust
#[monoio::main(driver = "fusion")]
async fn main() { // todo }
```
使用这种方式，你可以将 `fusion`、`legacy` 或 `uring` 作为 `driver` 参数。其中 `legacy` 和 `uring` 会强制使用 Legacy 和 Uring 作为 IO 驱动方式。在你明确知道编译出的二进制的运行平台时，可以使用这种方式。

使用 `fusion` 作为 `driver` 参数，会在运行时（启动时）动态探测平台的 io_uring 的支持情况，并优先选用 io_uring 作为 IO 驱动方式（如果你不指定 driver，这种是默认行为）。

第二种方式是通过代码指定：
```rust
monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
    .enable_timer()
    .build()
    .expect("Failed building the Runtime")
    .block_on(async move {
        // todo
    })
```
`RuntimeBuilder` 的泛型参数可以选择 `FusionDriver`、`IoUringDriver` 或 `LegacyDriver`。

第三种是通过 `start` 方法快速启动：
```rust
monoio::start::<monoio::LegacyDriver, _>(
    async move { // todo }
);
```
这种方式下，泛型参数可以指定为 `IoUringDriver` 或 `LegacyDriver`。

## Feature 选择
默认情况下我们已经打开了 `iouring` 和 `legacy` 这两个 feature。
如果你关闭了默认 feature 手动开启，请注意这两个 feature 务必开启至少一个。

1. 仅开启 `iouring` 时，`FusionDriver`（等价于 `IoUringDriver`）、`IoUringDriver` 可用
2. 仅开启 `legacy` 时，`FusionDriver`（等价于 `LegacyDriver`）、`LegacyDriver` 可用
3. 两个 feature 都开启时，`FusionDriver` 的运行时动态选择功能才可用