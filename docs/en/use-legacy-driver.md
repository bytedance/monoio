---
title: Use Legacy Driver
date: 2022-06-14 21:00:00
author: ihciah
---

Although Monoio's target platform is Linux that supports io_uring, you can use the Legacy driver when you have no control over this; or when you want to migrate smoothly; or when you want to develop on macOS.

Legacy drivers currently support macOS and Linux, based on kqueue and epoll respectively.

## Boot Options
The first way to configure is through macros:
```rust
#[monoio::main(driver = "fusion")]
async fn main() { // todo }
```
In this way, you can pass `fusion`, `legacy` or `uring` as the `driver` parameter. Among them, `legacy` and `uring` will force the use of Legacy and Uring as the IO driver. You can use this method when you know exactly what platform the compiled binary will run on.

Using `fusion` as the `driver` parameter will detect the platform's io_uring support at runtime (on startup), and prefer io_uring as the IO driver.
Note that if you do not specify the driver, `fusion` mode will be used by default.

The second way is to specify by code:
```rust
monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
    .enable_timer()
    .build()
    .expect("Failed building the Runtime")
    .block_on(async move {
        // todo
    })
```
The generic parameter of `RuntimeBuilder` can choose `FusionDriver`, `IoUringDriver` or `LegacyDriver`.

The third is to quickly start through the `start` method：
```rust
monoio::start::<monoio::LegacyDriver, _>(
    async move { // todo }
);
```
In this way, the generic parameter can be specified as `IoUringDriver` or `LegacyDriver`.

## How to Choose Feature
By default we have turned on the `iouring` and `legacy` features.

If you turn off the default features and turn it on manually, please note that at least one of these two features must be turned on.

1. When only `iouring` is enabled, `FusionDriver` (equivalent to `IoUringDriver`) and `IoUringDriver` are available
2. When only `legacy` is enabled, `FusionDriver` (equivalent to `LegacyDriver`) and `LegacyDriver` are available
3. When both features are enabled, using `FusionDriver` can select io driver dynamically.
