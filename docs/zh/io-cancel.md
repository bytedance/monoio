---
title: 取消 IO
date: 2023-02-27 15:09:00
author: ihciah
---

# 取消 IO
为了支持取消 IO / 带超时 IO，我们提供了一套新的 IO Trait。

本文给出了使用样例与设计文档。

## 如何使用
以读为例，下面这段代码定义了可取消的异步读 Trait：
```rust
/// CancelableAsyncReadRent: async read with a ownership of a buffer and ability to cancel io.
pub trait CancelableAsyncReadRent: AsyncReadRent {
    /// The future of read Result<size, buffer>
    type CancelableReadFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoBufMut + 'a;
    /// The future of readv Result<size, buffer>
    type CancelableReadvFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoVecBufMut + 'a;

    fn cancelable_read<T: IoBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadFuture<'_, T>;

    fn cancelable_readv<T: IoVecBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadvFuture<'_, T>;
}
```

在做带超时的 IO 时，可以这么使用该 Trait：
```rust
let mut buf = vec![0; 1024];

let canceler = monoio::io::Canceller::new();
let handle = canceler.handle();

let timer = monoio::time::sleep(std::time::Duration::from_millis(100));
monoio::pin!(timer);
let recv = conn.cancelable_read(buf, handle);
monoio::pin!(recv);

monoio::select! {
    _ = &mut timer => {
        canceler.cancel();
        let (res, _buf) = recv.await;
        if matches!(res, Err(e) if e.raw_os_error() == Some(125)) {
            // Canceled success
            buf = _buf;
            todo!()
        }
        // Canceled but executed
        // Process res and buf
        todo!()
    },
    r = &mut recv => {
        let (res, _buf) = r;
        // Process res and buf
        todo!()
    }
}
```

在实现 IO 封装时，透传 CancelHandle 即可。

## 现状与问题
当前接口会拿 Buffer 所有权并生成 Future，当 Future 结束时返回 Result 和 Buffer。

转移所有权有其必要性：Kernel 使用 buffer 的时机不确定，所以需要在 op 返回之前持续保证 buffer 有效性。如果 buffer 在用户手上那么用户完全可以 drop，就会导致错误的内存读写。

但转移所有权也引入了问题：Buffer 只能在 Future 结束后才能转移到用户侧。当用户想要实现超时 IO 时，无法拿回 Buffer；当前实现的另一个问题是，即便在开启了 `async-cancel` feature 的情况下，直接 Drop Future 后 Runtime 对 IO 的取消操作也仅为尽力保证取消（在成功推入 CancelOp 并被 Kernel 处理前，IO 仍有机会完成），可能会导致超时读时数据丢失等问题，继而导致数据流上的数据错误。总结一下就是这么两个问题：
1. 取消 IO 后会丢失 Buffer 所有权
2. 取消 IO 并不能确定性地被取消（这个是 io_uring 本身属性导致），可能会导致数据流读写错误

## 解决方案
1. 在 Buffer Drop 时通过其 Trait 暴露的接口转移所有权
    在 `Buf`/`BufMut` trait 上添加一个方法：`cancel(self)`，在 runtime 内部 drop buffer 时，将 buffer 所有权转移进去。
    在用户侧实现 `Buf`/`BufMut` 时，利用共享所有权(`Rc<RefCell<Option<T>>>`)维护一个 slot，能够用与 buffer 配对的一个 handler 结构拿回 buffer。

    但这个方案只解决 Buffer 所有权回收问题：比如 tcp accept 的取消依旧可能导致 drop connection；read 依旧可能丢失数据。显然这并不是一个理想的方案。

2. 暴露显式取消接口，通过原 Future 回收 Buffer 并感知 Result
暴露一个 Cancel 机制，能够主动触发 Future 返回。之后就可以直接 await Future 并在较短时间内拿到对应的 Result 和 Buffer。实现上也有几种方案：
    1. 返回一个 CancelHandler：在哪返回 CancelHandler 是个问题，当实现 `write_all` 的时候如何实现 CancelHandler 也是个问题。
    2. 传入一个 CancelHandler：有助于解决实现 wrapper 时的问题（可以直接透传这个结构进去，类似 golang `Context`），但依旧存在问题：接口怎么定，怎么把 CancelHandler 丢进去？
        1. 新增一个独立的 Trait
        2. 在现有 Trait 内扩展新的接口，并默认空实现
        3. 修改现有接口

    考虑到兼容性，现有接口尽量不做修改；扩展现有 Trait 依旧会引入兼容性问题，并会在用户未实际使用时导致不必要的代码实现。所以我们决定新增一个 Trait，并在其对应的 fn 上传递 CancelHandler。

3. 鸵鸟战术
    有 Cancel 需求时走 readable / writable，这个没副作用，随便取消。不过即便是走这种方案，仍需要将 Readable/Writable 抽成 trait。

    缺点：不利于对复合操作做 timeout（如 write_all / read_exact），且性能不佳。该方案与前面的方案并不矛盾，用户可以直接这么使用。

## 实现细节
对于不同 Driver 有不同的逻辑：
1. uring：推入 CancelOp，Kernel 消费后原 Future 返回。
2. legacy：新增定义 READ_CANCELED 和 WRITE_CANCELED 两种标记位，在目标被 cancel 时标记并 wake；任务 poll 时判定标志位，如果是 CANCELED 则直接返回 Operation Canceled 错误。
