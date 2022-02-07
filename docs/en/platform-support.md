---
title: Platform Support
date: 2021-11-24 20:00:00
author: ihciah
---

# Platform Support

Currently only linux with io_uring is supported, 5.6+ should be ok.

## Plans
Later we will support epoll/kqueue by using mio as **fallback**. If you do not use io_uring, there will be less meaningful to use Monoio.

IO interfaces with ownership of buffer is hard to use but has better performance in io_uring mode. So if you have io_uring in most of your environment, and care performance much, using Monoio is a good choice.

Windows is a little bit hard to support. We do not consider support it yet.
