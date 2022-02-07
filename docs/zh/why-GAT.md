---
title: 为什么使用 GAT
date: 2021-11-24 20:00:00
author: ihciah
---

# 为什么使用 GAT

我们全局开启了 GAT 并广泛地使用了它。

我们在 trait 中的关联类型 Future 上定义了生命周期，这样它就可以捕获 `&self` 而不是非要 `Clone self` 中的部分成员，或者单独定义一个带生命周期标记的结构体。

## 定义 Future
如何定义一个 Future？常规我们需要定义一个结构体，并为它实现 Future trait。这里的关键在于要实现 `poll` 函数。这个函数接收 `Context` 并同步地返回 `Poll`。要实现 `poll` 我们一般需要手动管理状态，写起来十分困难且容易出错。

这时你可能会说，直接 `async` 和 `await` 不能用吗？事实上 `async` 块确实生成了一个状态机，和你手写的差不多。但是问题是，这个生成结构并没有名字，所以如果你想把这个 Future 的类型用作关联类型就难了。这时候可以开启 `type_alias_impl_trait` 然后使用 opaque type 作为关联类型；也可以付出一些运行时开销，使用 `Box<dyn Future>`。

## 生成 Future
除了使用 `async` 块外，常规的方式就是手动构造一个实现了 `Future` 的结构体。这种 Future 有两种：
1. 带有所有权的 Future，不需要额外写生命周期标记。这种 `Future` 和其他所有结构体都没有关联，如果你需要让它依赖一些不 `Copy` 的数据，那你可以考虑使用 `Rc` 或 `Arc` 之类的共享所有权的结构。
2. 带有引用的 Future，这种结构体本身上就带有生命周期标记。例如，Tokio 中的 `AsyncReadExt`，`read` 的签名是 `fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Read<'a, Self>`。这里构造的 `Read<'a, Self>` 捕获了 self 和 buf 的引用，相比共享所有权，这是没有运行时开销的。但是这种 Future 不好作为 trait 的 type alias，只能开启 `generic_associated_types` 和 `type_alias_impl_trait`，然后使用 opaque type。

## 定义 IO trait
通常，我们的 IO 接口要以 `poll` 形式定义（如 `poll_read`），任何对 IO 的包装都应当基于这个 trait 来做（我们暂时称之为基础 trait）。

但是为了用户友好的接口，一般会提供一个额外的 `Ext` trait，主要使用其默认实现。`Ext` trait 为所有实现了基础 trait 的类自动实现。例如，`read` 返回一个 Future，显然基于这个 future 使用 `await` 要比手动管理状态和 `poll` 更容易。

那为什么基础 trait 使用 `poll` 形式定义呢？不能直接一步到位搞 Future 吗？因为 poll 形式是同步的，不需要捕获任何东西，容易定义且较为通用。如果直接一步到位定义了 Future，那么，要么类似 `Ext` 一样直接把返回 Future 类型写死（这样会导致无法包装和用户自行实现，就失去了定义 trait 的意义），要么把 Future 类型作为关联类型（前面说了，不开启 GAT 没办法带生命周期，即必须 static）。

所以总结一下就是，在目前的 Rust 稳定版本中，只能使用 poll 形式的基础 trait + future 形式的 Ext trait 来定义 IO 接口。

在开启 GAT 后这件事就能做了。我们可以直接在 trait 的关联类型中定义带生命周期的 Future，就可以捕获 self 了。

```rust
trait AsyncReadRent {
    type ReadFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;
    fn read<T: IoBufMut>(&self, buf: T) -> Self::ReadFuture<'_, T>;
}
```

这是银弹吗？不是。唯一的问题在于，如果使用了 GAT 这一套模式，就要总是使用它。如果你在 `poll` 形式和 GAT 形式之间反复横跳，那你会十分痛苦。基于 `poll` 形式接口自行维护状态，确实可以实现 Future（最简单的实现如 `poll_fn`）；但反过来就很难受了：你很难存储一个带生命周期的 Future。虽然使用一些 unsafe 的 hack 可以做(也有 cost)这件事，但是仍旧，限制很多且并不推荐这么做。`monoio-compat` 基于 GAT 的 future 实现了 Tokio 的 `AsyncRead` 和 `AsyncWrite`，如果你非要试一试，可以参考它。
