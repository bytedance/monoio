#![allow(missing_docs)]

use std::{
    cell::RefCell,
    future::Future,
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use crate::{
    runtime::spawn_without_static,
    task::{waker_fn::dummy_waker, JoinHandle},
};

struct ScopeData {
    handles_num: usize,
}

pub struct Scope<'scope, 'env: 'scope> {
    waker: Waker,
    data: RefCell<ScopeData>,
    _scope: PhantomData<&'scope mut &'scope ()>,
    _pinned: PhantomPinned,
    _env: PhantomData<&'env mut &'env ()>,
}

pub struct ScopedJoinHandle<'scope, T> {
    handle: JoinHandle<T>,
    _scope: PhantomData<&'scope ()>,
}

impl<'scope, T> Future for ScopedJoinHandle<'scope, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.handle) }.poll(cx)
    }
}

impl<'scope> Scope<'scope, '_> {
    pub fn spawn<F>(&'scope self, future: F) -> ScopedJoinHandle<'scope, F::Output>
    where
        F: 'scope + Future,
        F::Output: 'scope,
    {
        struct ScopedFuture<'scope, IF: Future> {
            future: IF,
            scope: &'scope RefCell<ScopeData>,
            waker: &'scope Waker,
        }

        impl<'scope, IF: Future> Future for ScopedFuture<'scope, IF> {
            type Output = IF::Output;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let this = unsafe { self.get_unchecked_mut() };
                let polled = unsafe { Pin::new_unchecked(&mut this.future) }.poll(cx);
                if let Poll::Ready(_) = polled {
                    let data = &mut this.scope.borrow_mut().handles_num;
                    *data -= 1;
                    if *data == 0 {
                        this.waker.wake_by_ref();
                    }
                }
                polled
            }
        }

        self.data.borrow_mut().handles_num += 1;
        ScopedJoinHandle {
            handle: unsafe {
                spawn_without_static(ScopedFuture {
                    future,
                    scope: &self.data,
                    waker: &self.waker,
                })
            },
            _scope: PhantomData,
        }
    }
}

pub async fn scope<'scope, 'env: 'scope, F, T>(f: F) -> T::Output
where
    F: FnOnce(&'scope Scope<'scope, 'env>) -> T,
    T: Future,
{
    struct ScopeFuture<'scope, 'env: 'scope, F, T>
    where
        F: FnOnce(&'scope Scope<'scope, 'env>) -> T,
        T: Future,
    {
        scope: Scope<'scope, 'env>,
        main: MaybeUninit<F>,
        main_future: Option<T>,
        main_ready: Option<MaybeUninit<T::Output>>,
        _pinned: PhantomPinned,
    }

    impl<'scope, 'env: 'scope, F, T> Future for ScopeFuture<'scope, 'env, F, T>
    where
        F: FnOnce(&'scope Scope<'scope, 'env>) -> T,
        T: Future,
    {
        type Output = T::Output;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            unsafe {
                let this = self.get_unchecked_mut();

                if let None = this.main_ready {
                    if let None = this.main_future {
                        let main = this.main.assume_init_read();
                        this.main_future = Some(main(std::mem::transmute(&this.scope as *const _)));
                    }

                    let future = this.main_future.as_mut().unwrap();
                    match Pin::new_unchecked(future).poll(cx) {
                        Poll::Ready(r) => {
                            this.main_ready = Some(MaybeUninit::new(r));
                        }
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                    }
                }

                if this.scope.data.borrow().handles_num == 0 {
                    return Poll::Ready(this.main_ready.as_ref().unwrap().assume_init_read());
                }

                this.scope.waker = cx.waker().clone();
                Poll::Pending
            }
        }
    }

    ScopeFuture {
        scope: Scope {
            waker: dummy_waker(),
            data: RefCell::new(ScopeData { handles_num: 0 }),
            _scope: PhantomData,
            _pinned: PhantomPinned,
            _env: PhantomData,
        },
        main: MaybeUninit::new(f),
        main_ready: None,
        main_future: None,
        _pinned: PhantomPinned,
    }
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::scope;
    use crate::{time::sleep, IoUringDriver, RuntimeBuilder};

    #[test]
    fn scope_ok() {
        let mut rt = RuntimeBuilder::<IoUringDriver>::new()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let a = 1;
            scope(|scope| async {
                sleep(Duration::from_millis(10)).await;
                assert_eq!(a, 1);
                scope.spawn(async {
                    sleep(Duration::from_millis(20)).await;
                    assert_eq!(a, 1);
                });
            })
            .await;
            drop(a);
            sleep(Duration::from_millis(30)).await;
        });
    }

    #[test]
    #[allow(unreachable_code)]
    fn skip_scope_should_not_execute() {
        let mut rt = RuntimeBuilder::<IoUringDriver>::new()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let a = 1;
            let _ = scope(|scope| async {
                unreachable!();
                sleep(Duration::from_millis(10)).await;
                assert_eq!(a, 1);
                scope.spawn(async {
                    sleep(Duration::from_millis(20)).await;
                    assert_eq!(a, 1);
                });
            });
            drop(a);
            sleep(Duration::from_millis(30)).await;
        });
    }
}
