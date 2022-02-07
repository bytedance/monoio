---
title: Performance test and comparison
date: 2021-12-01 15:50:00
author: ihciah
---

# Performance test data and comparison

In order to measure the performance of Monoio, we selected two more representative Runtimes to compare with Monoio: Tokio and Glommio.

## Testing environment
Our test is carried out on the ByteDance production network, and the client end and the server end are running on different physical machines.

Server information:
> Intel(R) Xeon(R) Gold 5118 CPU @ 2.30GHz
>
> Ethernet controller: Intel Corporation Ethernet Controller X710 for 10GbE SFP+ (rev 02)
>
> Linux 5.15.4-arch1-1 #1 SMP PREEMPT Sun, 21 Nov 2021 21:34:33 +0000 x86_64 GNU/Linux
>
> rust nightly-2021-11-26

## Testing tools
The testing tool is developed based on Monoio using Rust.

You can find its source code [here](https://github.com/monoio-rs/monoio-benchmark).

## Testing data

### Extreme performance testing
In this test we will start a fixed number of connections on the client side. The more connections, the higher the load on the server. This test aims to detect the extreme performance of the system.

1 Core                     |  4 Cores
:-------------------------:|:-------------------------:
![1core](/.github/resources/benchmark/monoio-bench-1C.png)  |  ![4cores](/.github/resources/benchmark/monoio-bench-4C.png)

8 Cores                     |  16 Cores
:-------------------------:|:-------------------------:
![8cores](/.github/resources/benchmark/monoio-bench-8C.png)  |  ![16cores](/.github/resources/benchmark/monoio-bench-16C.png)

In the case of a single core and very few connections, Monoio's latency will be higher than Tokio, resulting in lower throughput than Tokio. This latency difference is due to the difference between io_uring and epoll.

Except for the previous scenario, Monoio performance is better than Tokio and Glommio. Tokio will decrease the average peak performance of a single core as the number of cores increases; Monoio's peak performance has the best horizontal scalability.

Under single core, Monoio's performance is slightly better than Tokio; under 4 cores, the peak performance is about twice that of Tokio; under 16 cores, it is close to 3 times. Glommio and the model are the same as Monoio, so it also has good horizontal scalability, but its peak performance is still a certain gap compared to Monoio.

![100B](/.github/resources/benchmark/monoio-bench-100B.png)
We use a message size of 100Byte to test the peak performance under different core counts (1K will fill up the network card when there are more cores). It can be seen that Monoio and Glommio can maintain linearity better; while Tokio has very little performance improvement or even degradation when there are more cores.

### Fixed pressure testing
In the production environment, it is impossible for us to hit the full server side, so testing the performance under constant pressure is also of great significance.

1 Core * 80 Connections    |  4 Cores * 80 Connections
:-------------------------:|:-------------------------:
![1core*80](/.github/resources/benchmark/monoio-bench-1C-80conn-qps.png)  |  ![4cores*80](/.github/resources/benchmark/monoio-bench-4C-80conn-qps.png)

1 Cores * 250 Connections  |  4 Cores * 250 Connections
:-------------------------:|:-------------------------:
![1core*250](/.github/resources/benchmark/monoio-bench-1C-250conn-qps.png)  |  ![4cores*250](/.github/resources/benchmark/monoio-bench-4C-250conn-qps.png)

Similar to the problem explained by the previous test data, Tokio has a delay advantage over uring-based Glommio and Monoio when the number of connections is small. But Monoio is still the lowest in CPU consumption.

As the number of connections increases, Monoio has the lowest latency and CPU usage.

## Reference data
You can find more specific benchmark data in [the link](/.github/resources/benchmark/raw_data.txt).
