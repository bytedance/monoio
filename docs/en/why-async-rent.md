---
title: Async Rent
date: 2021-11-24 20:00:00
author: ihciah
---

# Async Rent

We use async rent as our core IO abstraction.

## The need of io_uring
1. Address of the buffer being fixed.

    Since we only submit the buffer to the kernel, we don't know when the buffer will be read or written. We must make sure the address is fixed, so it can be valid when operated.

2. Lifetime promise of the buffer.

    Consider the following circumstance:
    1. User creates a buffer.
    2. User takes a reference of the buffer and use the reference(no matter `&` or `&mut`) to do read or write.
    3. Instead of `await` the future, the user just drops it.
    4. Since the buffer has no borrower, the user can drop the buffer safely.
    5. The `ReadOp` or `WriteOp` is still in the queue or being processed. We can not guarantee the `CancelOp` is successfully pushed and consumed in time.
    6. The kernel is operating on invalid memory!

So, we want to make sure the buffer is fixed and valid during the operation. We found no way to do this if not take ownership of the buffer.

Why Tokio AsyncIO do not require buffer ownership? Answer in a simple way: Because we can cancel it instantly. We do async read or async write with the borrow of the buffer, then we can generate future with the borrow, and the rust compiler know their relation. Once the future is done or dropped, the borrow is dropped, and the kernel will not operate on the buffer(because with epoll+syscall, syscall is executed synchronously). In another word, if we can do async drop, we can do async IO with the reference of the buffer with io_uring instead of ownership.

## Ecology Problem
The biggest problem is the compatibility with the current async ecology.

To relieve the problem, we provide a compatible wrapper for structs like `TcpStream`. With the wrapper, users can use Tokio AsyncIO with data copy cost.

And also, if user uses BufRead or BufWrite, there will be no need to take the ownership of the buffer since we will copy the data instantly. We can use Tokio AsyncIO now too.
