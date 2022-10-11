---
title: 与 Tokio 生态兼容
date: 2021-12-17 18:00:00
author: ihciah
---

# 与 Tokio 生态兼容
现有 Rust 组件中有大量的组件与 Tokio 是兼容的，它们直接依赖了 Tokio 的 `AsyncRead` 和 `AsyncWrite` 接口。

而在 Monoio，由于底层是异步系统调用，所以我们选择了类似 tokio-uring 的做法：提供传 buffer 所有权的 IO 接口。但现阶段明显没有很多可用的库可以工作，所以我们需要以一定的性能牺牲来快速支持功能。

## monoio-compat
`monoio-compat` 是一个兼容层，它基于 buffer 所有权的接口实现 Tokio 的 `AsyncRead` 和 `AsyncWrite`。

### 工作原理
对于 `poll_read`，这里会先读到用户传递的 slice 的剩余容量，之后将自己持有的 buffer 限制为该容量并生成 future。之后每次用户再次 `poll_read`，都会转发到这个 future 的 `poll` 方法。在返回 `Ready` 时将 buffer 的内容拷贝到用户传递的 slice 中。

对于 `poll_write`，会将用户传递 slice 的内容先拷贝至自有 buffer，之后生成 future 并存储，并立刻返回 Ready。之后每次用户再次 `poll_write`，都会首先等待上一次的内容发送完毕，之后再拷贝数据并立刻返回 Ready。行为上有种类似 BufWriter 的效果，会导致 error 被延迟感知。

代价上，使用这个兼容层会让你额外付出一次 buffer 拷贝开销。

### 使用限制
对于写操作，用户需要在最后手动调用 AsyncWrite 的 poll_flush 或 poll_shutdown（或 AsyncWriteExt 的对应 flush 或 shutdown 方法），否则数据有可能不会被提交至内核（连续的写入不需要手动 flush）。

## 面向 poll 的接口与异步系统调用
Rust 中有两种表达异步的方式，一种是基于 `poll` 的，一种是基于 `async` 的。`poll` 是同步的，语义上表达立即尝试；而 `async` 本质上是 poll 的语法糖，它会吞掉 future 并生成状态机，await 的时候是在循环执行这个状态机。

在 Tokio 中，有类似 `poll_read` 和 `poll_write` 的方法，它们都表达了同步尝试这个语义。

当返回 `Pending` 时意味着 IO 未就绪（并给 cx 的 waker 注册了通知），当返回 `Ready` 时意味着 IO 已经完成了。基于同步系统调用实现这两个接口很容易，直接做对应系统调用并判断返回结果，如果 IO 未就绪则挂起到 Reactor。

然而在异步系统调用下这两个接口很难实现。如果我们已经将一个 Op 推入 io_uring SQ，那么在消费到对应 CQE 之前，这次 syscall 的状态是不确定的。我们没有办法提供明确的已完成或未完成的语义。在 `monoio-compat` 中我们通过一些 hack 提供了 poll-like 的接口，所以能力的缺失导致了我们的使用限制。在异步系统调用下，传递 buffer 所有权并配合 `async+await` 是更合适的。

目前 Rust 标准库并没有通用的面向异步系统调用的接口，相关组件生态也没有。我们正在努力改善这个问题。