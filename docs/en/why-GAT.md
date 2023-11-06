---
title: Why GAT
date: 2021-11-24 20:00:00
updated: 2023-11-06 16:49:00
author: ihciah
---

# Why GAT

We enable GAT globally and use it widely.

We define the lifetime of the associated Future so it can capture `&self` instead of `Clone`ing part of it, or defining another struct with lifetime mark.

## Define a future
How to define a future? Normally, we need to define a `poll` function. The function take `Context` and return `Poll` synchronously. To implement that, we have to manually manage the state of the future, and it\'s a hard task and easy to make mistake.

Maybe you think, we just use `async` and `await` then it will be fine! Infact, `async` block will generate the state machine for you. However, you can\'t name it, which is hard to use when we want to use the future as the associated type of another struct, for example, a tower `Service`. In this case, you need opaque type and you have to enable `type_alias_impl_trait` feature; or, you can use `Box<dyn Future>` with some runtime cost.

## Generate a future
Except for using `async` block, we can manually construct a struct which implements `Future`. There are two common types:
1. Future with ownership, which means there\'s no need to define lifetime mark in struct. Generate the future, and the future has no relationship with any others. Maybe you have to share ownership with `Rc` or `Arc`.
2. Future with reference. You have to define lifetime mark in struct. For example, in Tokio `AsyncReadExt`, the `read` is like `fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Read<'a, Self>`. The constructed struct capture the reference of `self` and `buf` without cost of sharing ownership. However, without GAT this type of future cannot be used in type alias.

## Define IO trait
In a normal way, we have to define the trait in a `poll` style like `poll_read`. Any wrapping should done in `poll` style and implement the trait.

For user-friendliness, there will be another `Ext` trait with default implementation. The `Ext` trait requires and only requires the former trait. `Ext` trait provides something like `read` to return a future. Using `await` on the future is more convenient than manage state and do `poll` manually. The future returned is a manually defined struct(with or without lifetime mark) implementing `Future`.

Why using `poll` style for the basic trait? Because `poll` style is synchronous and always returns `Poll`, which does not capture any thing, is easy to define and generic enough.

With GAT things will be easier. How about generate a future directly in trait? With GAT we can define a type alias with lifetime in trait. Then we can return a future capturing the reference of self.
```rust
trait AsyncReadRent {
    type ReadFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;
    fn read<T: IoBufMut>(&self, buf: T) -> Self::ReadFuture<'_, T>;
}
```

The only problem here is, if you use GAT style, you should always use it. Providing `poll` style based on GAT is not easy. As an example, `monoio-compat` implement tokio `AsyncRead` and `AsyncWrite` based on GAT style future with some unsafe hack(and also with a `Box` cost).

## async_fn_in_trait
`async_fn_in_trait` and `return_position_impl_trait_in_trait` is stable now in rust and can be used to replace GAT usage here(related [issue](https://github.com/rust-lang/rust/issues/91611)).

Now we can define and impl async trait easier：
```rust
trait AsyncReadRent {
    fn read<T: IoBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>>;
}

impl AsyncReadRent for Demo {
    async fn read<T: IoBufMut>(&mut self, buf: T) -> BufResult<usize, T> { ... }
}
```
