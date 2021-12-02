---
title: 平台支持
date: 2021-11-24 20:00:00
author: ihciah
---

# 平台支持

目前仅支持 Linux 平台，且需要内核支持 io-uring，最低版本为 5.6。

## 未来计划
未来将会支持 epoll/kqueue 作为 **fallback**。注意，即便支持了 epoll/kqueue，也希望你能在多数场景下利用 io-uring，否则使用 Monoio 的意义不大。

Monoio 使用类似 Tokio-uring 的带有 buffer 所有权的 IO 接口，这套接口与 Tokio 不兼容，且需要组件单独适配支持。所以如果你的大部分运行环境支持 io-uring，且你十分注重性能，那么欢迎使用 Monoio。

Windows 支持起来较为困难，且开发意义和生产意义不大，目前暂无支持计划。
