# Monoio
一个基于 io_uring/epoll/kqueue 和 thread-per-core 模型 Rust Runtime。

[![Crates.io][crates-badge]][crates-url]
[![MIT/Apache-2 licensed][license-badge]][license-url]
[![Build Status][actions-badge]][actions-url]
[![Codecov][codecov-badge]][codecov-url]
[English Readme][en-readme-url]

[crates-badge]: https://img.shields.io/crates/v/monoio.svg
[crates-url]: https://crates.io/crates/monoio
[license-badge]: https://img.shields.io/crates/l/monoio.svg
[license-url]: LICENSE-MIT
[actions-badge]: https://github.com/bytedance/monoio/actions/workflows/ci.yml/badge.svg
[actions-url]: https://github.com/bytedance/monoio/actions
[codecov-badge]: https://codecov.io/gh/bytedance/monoio/branch/master/graph/badge.svg?token=3MSAMJ6X3E
[codecov-url]: https://codecov.io/gh/bytedance/monoio
[en-readme-url]: README.md

## 设计目标
作为一个基于 io_uring/epoll/kqueue 的 Runtime，Monoio 目标是在兼顾平台兼容性的情况下，做最高效、性能最优的 thread-per-core Rust Runtime。

我们的出发点很简单：跨线程任务调度会带来额外开销，且对 Task 本身有 `Send` 和 `Sync` 约束，导致无法很好地使用 thread local storage。而很多场景并不需要跨线程调度。如 `nginx` 这种负载均衡代理，我们往往可以以 thread-per-core 的模式编写。这样可以减少跨线程通信的开销，提高性能；也可以尽可能地利用 thread local 来做极低成本的任务间通信。当任务不需要被跨线程调度时，它就没有了实现 `Send` 和 `Sync` 的约束。另一点是 io_uring 相比 epoll 在性能上有很大提升，我们也希望能够尽可能利用它达到最佳性能。所以基于 io_uring 做一套 thread-per-core 的 Runtime 理论上可以获得一些场景下的最佳性能。

Monoio 就是这样一个 Runtime：它并不像 Tokio 那样通过公平调度保证通用性，它的目标是在**特定场景下**(thread per core 模型并不适用于所有场景)提供最好的性能。为了性能，Monoio 还开启了 GAT 等一系列的 unstable feature；同时也提供了全新的无拷贝的 IO 抽象。

功能上目前支持了部分网络 IO 和计时器；也支持跨线程异步通信。

[我们的基准测试](docs/zh/benchmark.md) 表明 Monoio 比其他常见的 Rust 运行时具有更好的性能。

## 快速上手
要使用 Monoio，你需要最新的 nightly 工具链。如果你已经安装了 nightly 工具链，请确保是最新的版本。

在项目中创建 `rust-toolchain` 文件并在其中写入 `nightly` 即可强制指定；也可以使用 `cargo +nightly` 来构建或运行。

同时，如果你想使用 io_uring，你需要确保你当前的内核版本是较新的([5.6+](docs/zh/platform-support.md))；并且 memlock 是一个[合适的配置](docs/zh/memlock.md)。如果你的内核版本不满足需求，可以尝试使用 legacy driver 启动([参考这里](/docs/zh/use-legacy-driver.md))，当前支持 Linux 和 macOS。

🚧实验性的 windows 系统支持正在开发中。

这是一个非常简单的例子，基于 Monoio 实现一个简单的 echo 服务。运行起来之后你可以通过 `nc 127.0.0.1 50002` 来连接它。

```rust,no_run
/// A echo example.
///
/// Run the example and `nc 127.0.0.1 50002` in another shell.
/// All your input will be echoed out.
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::net::{TcpListener, TcpStream};

#[monoio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:50002").unwrap();
    println!("listening");
    loop {
        let incoming = listener.accept().await;
        match incoming {
            Ok((stream, addr)) => {
                println!("accepted a connection from {}", addr);
                monoio::spawn(echo(stream));
            }
            Err(e) => {
                println!("accepted connection failed: {}", e);
                return;
            }
        }
    }
}

async fn echo(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut res;
    loop {
        // read
        (res, buf) = stream.read(buf).await;
        if res? == 0 {
            return Ok(());
        }

        // write all
        (res, buf) = stream.write_all(buf).await;
        res?;

        // clear
        buf.clear();
    }
}
```

在本仓库的 `examples` 目录中有更多的例子。

## 限制
1. 在 Linux 5.6 或更新版本上，Monoio 可以以 uring 或 epoll 作为可选驱动方式，低版本 Linux 上只能以 epoll 方式运行，在 macOS 上可以使用 kqueue。其他平台暂不支持。
2. Monoio 这种 thread per core 的 runtime 并不适用于任意场景。如果负载非常不均衡，相比公平调度模型的 Tokio 它可能会性能变差，因为 CPU 利用可能不均衡，不能充分利用可用核心。

## 贡献者
<a href="https://github.com/bytedance/monoio/graphs/contributors"><img src="https://opencollective.com/monoio/contributors.svg?width=890&button=false" /></a>

在此表示感谢！

## 关联项目
- [local-sync](https://github.com/monoio-rs/local-sync)：一个线程内的 channel 实现
- [monoio-tls](https://github.com/monoio-rs/monoio-tls)：Monoio TLS 支持
- [monoio-codec](https://github.com/monoio-rs/monoio-codec)：Monoio Codec 支持

HTTP 框架和 RPC 框架在做了在做了(咕咕咕)。

## 协议
Monoio 基于 MIT 或 Apache 协议授权。

在开发中我们大量参考了 Tokio, Mio, Tokio-uring 和其他一些项目，在此向这些项目的贡献者们表示感谢。
