---
title: 为什么使用 AsyncRent 作为 IO 抽象
date: 2021-11-24 20:00:00
author: ihciah
---

# 为什么使用 AsyncRent 作为 IO 抽象

我们使用 AsyncRent 作为 IO 抽象。

## io-uring 的需要
1. buffer 的位置固定

因为我们只是将 buffer 提交给 kernel，我们不知道 kernel 什么时候会向其中写数据或从中读数据。我们必须保证在 IO 完成前，buffer 地址是有效的。

2. buffer 声明周期保证

    考虑下面这种情况：
    1. 用户创建了 Buffer
    2. 用户拿到了 buffer 的引用（不管是 `&` 还是 `&mut`）来做 read 和 write。
    3. Runtime 返回了 Future，但用户直接将其 Drop 了。
    4. 现在没有人持有 buffer 的引用了，用户可以直接将其 Drop 掉。
    5. 但是，buffer 的地址和长度已经被提交给内核，它可能即将被处理，也可能已经在处理中了。我们可以推入一个 `CancelOp` 进去，但是我们也不能保证 `CancelOp` 被立刻消费。
    6. Kernel 这时已经在操作错误的内存啦，如果这块内存被用户程序复用，会导致内存破坏。

所以，我们想要保证在 IO 完成前，buffer 地址固定且是有效的。在 Rust 中，如果不拿 buffer 的所有权，这件事几乎不可能做到。

为什么 Tokio 的 AsyncIO 不需要 buffer 所有权呢？原因很简单：根本原因是 kernel 对 buffer 的操作是同步的，可以立刻取消 IO。所以我们可以只拿 buffer 的引用去做读写，然后我们可以构造捕获这个 buffer 的 Future，然后 Rust 编译器会知道它们的生命周期和引用关系。一旦 Future 完成了或被 Drop 了，这个引用就释放了，这时 kernel 是不可能操作到 buffer 的（在 epoll+syscall 下，syscall 是同步执行的，一旦用户拿到控制权，那么一定 kernel 一定不在操作）。换句话说，如果我们可以做 async drop，我们基于 io-uring 也能使用类似 Tokio 的只需要 buffer 引用的 IO 接口。

## 生态问题
最大的问题在于生态，基于 buffer 所有权的 IO 接口和目前的业界生态不搭。

为了缓解这个问题，我们为 `TcpStream` 之类的结构类提供了一个兼容的包装。用户可以使用 Tokio 的接口操作这些结构，但要付出多一次数据拷贝的开销。

并且，如果用户使用 BufRead 或 BufWrite，也没有必要非要使用带有 buffer 所有权的接口操作，因为它的内部本来就有 buffer，我们可以在 call 的时候立刻拷贝数据从而不必将用户传入的 buffer 置于一个不确定的状态（指不确定什么时候它被读写），我们也可以在没有新增的开销的情况下提供类似 Tokio 的接口。
