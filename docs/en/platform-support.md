---
title: Platform Support
date: 2021-11-24 20:00:00
author: ihciah
---

# Platform Support

Linux and macOS are currently supported. On the Linux platform, we can use io_uring or epoll as the IO driver; on the macOS platform, we will use kqueue as the IO driver.

How to use Legacy driver can refer to [here](/docs/en/use-legacy-driver.md).

## Future Plans
Windows may be supported, but there are no plans for that in the short term.

If you want to develop on Windows and deploy on Linux, then you can try wsl.