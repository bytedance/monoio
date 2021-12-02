---
title: 性能测试与对比
date: 2021-12-01 15:50:00
author: ihciah
---

# 性能测试数据与对比

为了衡量 Monoio 的性能表现，我们选取了 2 个比较具有代表性的 Runtime 与 Monoio 做对比：Tokio 和 Glommio。

## 测试环境
我们的测试在字节跳动生产网上进行，发压端与被压端运行在不同的物理机上。

被压端配置：
> Intel(R) Xeon(R) Gold 5118 CPU @ 2.30GHz
>
> Ethernet controller: Intel Corporation Ethernet Controller X710 for 10GbE SFP+ (rev 02)
>
> Linux 5.15.4-arch1-1 #1 SMP PREEMPT Sun, 21 Nov 2021 21:34:33 +0000 x86_64 GNU/Linux
>
> rust nightly-2021-11-26

## 测试工具
测试工具使用 Rust 基于 Monoio 开发。

你可以在[这里](https://github.com/monoio-rs/monoio-benchmark)找到它的源代码。

## 测试数据

### 极限性能测试
在本测试中我们会在客户端启动固定数量的连接。连接数越多，服务端的负荷越高。本测试旨在探测系统的极限性能。

1 Core                     |  4 Cores
:-------------------------:|:-------------------------:
![1core](/.github/resources/benchmark/monoio-bench-1C.png)  |  ![4cores](/.github/resources/benchmark/monoio-bench-4C.png)

8 Cores                     |  16 Cores
:-------------------------:|:-------------------------:
![8cores](/.github/resources/benchmark/monoio-bench-8C.png)  |  ![16cores](/.github/resources/benchmark/monoio-bench-16C.png)

在单核且连接数极少的情况下，Monoio 的延迟会高于 Tokio，导致吞吐低于 Tokio。这个延迟差异是由于 io-uring 和 epoll 的不同导致。

除了前面的这种场景，Monoio 性能都好于 Tokio 和 Glommio。Tokio 会随着核数的增多，单 Core 的平均峰值性能出现较大下降；Monoio 的峰值性能的水平扩展性是最好的。

单核心下，Monoio 的性能略好于 Tokio；4 核心下峰值性能是 Tokio 的 2 倍左右；16 核心时接近 3 倍。Glommio 和模型和 Monoio 是一致的，所以也有较好的水平扩展性，但是它的峰值性能对比 Monoio 仍旧有一定差距。

![100B](/.github/resources/benchmark/monoio-bench-100B.png)
我们使用 100Byte 的消息大小测试不同核数下的峰值性能（1K 会在核数较多时打满网卡）。可以看出，Monoio 和 Glommio 可以较好地保持线性；而 Tokio 在核数较多时性能提升极少甚至出现劣化。

### 固定压力测试
生产环境中我们不可能打满服务端，因此测试固定压力下的性能表现也有很大意义。

1 Core * 80 连接            |  4 Cores * 80 连接
:-------------------------:|:-------------------------:
![1core*80](/.github/resources/benchmark/monoio-bench-1C-80conn-qps.png)  |  ![4cores*80](/.github/resources/benchmark/monoio-bench-4C-80conn-qps.png)

1 Cores * 250 连接           |  4 Cores * 250 连接
:-------------------------:|:-------------------------:
![1core*250](/.github/resources/benchmark/monoio-bench-1C-250conn-qps.png)  |  ![4cores*250](/.github/resources/benchmark/monoio-bench-4C-250conn-qps.png)

同前面的测试数据说明的问题类似，在连接数较小时，Tokio 相比 基于 uring 的 Glommio 和 Monoio 有延迟优势。但在 CPU 消耗上 Monoio 仍是最低的。

随着连接数的上涨，Monoio 在延迟和 CPU 占用上都是最低的。

## 参考数据
原始的压测数据可以在 [这里](/.github/resources/benchmark/raw_data.txt) 找到