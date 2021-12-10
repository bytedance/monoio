# Monoio
A thread-per-core Rust runtime with io_uring.

[![Crates.io][crates-badge]][crates-url]
[![MIT/Apache-2 licensed][license-badge]][license-url]
[![Build Status][actions-badge]][actions-url]
[中文说明][zh-readme-url]

[crates-badge]: https://img.shields.io/crates/v/monoio.svg
[crates-url]: https://crates.io/crates/monoio
[license-badge]: https://img.shields.io/crates/l/monoio.svg
[license-url]: LICENSE-MIT
[actions-badge]: https://github.com/bytedance/monoio/actions/workflows/ci.yml/badge.svg
[actions-url]: https://github.com/bytedance/monoio/actions
[zh-readme-url]: README-zh.md

## Design Goal
As a runtime based on io_uring, Monoio is designed to be the most efficient and performant Rust runtime.

For some use cases, it is not necessary to make task schedulable between threads. For example, if we want to implement a load balancer like nginx, we may want to write it in a thread-per-core way. The thread local data need not to be shared between threads, so the `Sync` and `Send` will not have to be implemented.

Also, the Monoio is designed to be efficient. To achieve this goal, we enabled many Rust unstable features like GAT; and we designed a whole new IO abstraction to avoid copying, which may cause some compatibility problems.

[Our benchmark](docs/en/benchmark.md) shows that Monoio has a better performance than other common Rust runtimes.

## Quick Start
To use monoio, you need the latest nightly rust toolchain. If you already installed it, please make sure it is the latest version.

To force using nightly, create a file named `rust-toolchain` and write `nightly` in it. Also, you can use `cargo +nightly` to build or run.

Here is a basic example of how to use Monoio.

```rust
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

async fn echo(stream: TcpStream) -> std::io::Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    loop {
        // read
        let (res, _buf) = stream.read(buf).await;
        buf = _buf;
        let res: usize = res?;
        if res == 0 {
            return Ok(());
        }

        // write all
        let (res, _buf) = stream.write_all(buf).await;
        buf = _buf;
        res?;

        // clear
        buf.clear();
    }
}
```

## Limitations
1. Since we rely on io_uring, currently monoio depends on Linux 5.6 or later. Epoll or other multiplex I/O will be supported soon.
2. Monoio can not solve all problems. If the workload is very unbalanced, it may cause performance degradation than Tokio since CPU cores may not be fully utilized.

## Contributors
![GitHub Contributors Image](https://contrib.rocks/image?repo=bytedance/monoio)

Thanks for their contributions!

## Licenses
Monoio is licensed under the MIT license or Apache license.

During developing we referenced a lot from Tokio, Mio, Tokio-uring and other related projects. We would like to thank the authors of these projects.
