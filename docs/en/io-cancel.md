---
title: Cancel IO
date: 2023-02-27 15:09:00
author: ihciah
---

# Cancel IO
To support Cancel IO / IO with timeout, we provide a new set of IO Trait.

This article gives a sample usage and design document.

## How to use
Taking reads as an example, the following code defines a cancelable asynchronous read Trait.
```rust
/// CancelableAsyncReadRent: async read with an ownership of a buffer and ability to cancel io.
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

When doing IO with timeouts, the Trait can be used like this.
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

When implementing IO wrapping, just pass through the CancelHandle.

## Status and issues
The current interface takes ownership of the Buffer and generates the Future, returning the Result and Buffer when the Future ends.

There is a need to transfer ownership: the Kernel uses the buffer at an uncertain time, so it needs to keep the buffer valid until the op returns. If the buffer is in the hands of the user, then the user can drop it, which can lead to incorrect memory reads and writes.

However, transferring ownership also introduces a problem: the buffer can only be transferred to the user side after the Future is finished. Another problem with the current implementation is that even with the `async-cancel` feature enabled, the Runtime's cancellation of IO after a direct Drop Future is only as good as it can be (before it is successfully pushed into CancelOp and processed by the Kernel). This may lead to problems such as data loss during timeout reads, which in turn may lead to data errors on the data stream. To summarize, there are two problems: 1.
1. Cancellation of IO will result in loss of Buffer ownership
2. Cancellation of IO is not deterministically cancelled (this is due to the io_uring property itself), which may lead to read and write errors on the data stream

## Solution
1. Transfer ownership through the interface exposed by the Trait when the Buffer is dropped
    Add a method to the `Buf`/`BufMut` trait: `cancel(self)` to transfer the ownership of the buffer when it is dropped inside the runtime.
    When implementing `Buf`/`BufMut` on the user side, a slot is maintained using shared ownership (`Rc<RefCell<Option<T>>>`), and the buffer can be retrieved using a handler structure paired with the buffer.

    But this solution only solves the Buffer ownership recovery problem: for example, a tcp accept cancellation may still result in a drop connection; a read may still lose data. Obviously, this is not an ideal solution. 2.

2. Expose an explicit cancellation interface to recover Buffer and sense the Result through the original Future
Expose a Cancel mechanism to trigger the return of Future. There are also several options for implementation: 1.
    1. return a CancelHandler: where to return the CancelHandler is a problem, and how to implement the CancelHandler when implementing `write_all` is also a problem. 2.
    2. pass in a CancelHandler: helps to solve the problem when implementing wrapper (you can pass this structure in directly, similar to golang `Context`), but there are still problems: how to define the interface, how to throw in the CancelHandler?
        1. add a separate Trait
        2. extend the new interface within the existing Trait and implement it empty by default
        3. modify the existing interface

    Considering the compatibility, we try not to modify the existing interface; extending the existing Trait will still introduce compatibility issues and will lead to unnecessary code implementation when the user does not actually use it. So we decided to add a new Trait and pass CancelHandler on its corresponding fn.

3. Ostrich tactics
    When there is a need for Cancel, we go for readable / writable, which has no side effects and can be cancelled at will. However, even if you go this way, you still need to abstract the Readable/Writable into a trait.

    Disadvantages: not conducive to compound operations to do timeout (such as write_all / read_exact), and poor performance. This solution is not contradictory to the previous one, and users can use it directly that way.

## Implementation details
There are different logics for different Drivers.
1. uring: push in CancelOp, Kernel consumes and returns the original Future. 2. legacy: add a definition of READ.
2. legacy: new definition of READ_CANCELED and WRITE_CANCELED two marker bits, mark and wake when the target is canceled; task poll when determining the marker bit, if it is CANCELED then directly return Operation Canceled error.
