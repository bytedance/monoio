---
title: 平台支持
date: 2021-11-24 20:00:00
author: ihciah
---

# 平台支持

目前支持 Linux 和 macOS。在 Linux 平台上，我们可以使用 io_uring 或 epoll 作为 IO 驱动；在 macOS 平台上，我们会使用 kqueue 作为 IO 驱动。

如何使用 Legacy 驱动可以参考[这里](/docs/zh/use-legacy-driver.md)。

## 未来计划
可能会支持 Windows，但短期内暂无该计划。

如果你想在 Windows 开发，在 Linux 部署，那么你可以尝试使用 wsl。
