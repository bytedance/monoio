use scoped_tls::scoped_thread_local;

use crate::driver::Driver;
use crate::scheduler::{LocalScheduler, TaskQueue};
// use crate::task::task_impl::spawn_local;
use crate::task::waker_fn::{dummy_waker, set_poll, should_poll};
use crate::task::{new_task, JoinHandle};
use crate::time::driver::Handle as TimeHandle;

use std::future::Future;

scoped_thread_local!(pub(crate) static CURRENT: Context);

pub(crate) struct Context {
    /// Thread id(not the kernel thread id but a generated unique number)
    #[cfg(feature = "sync")]
    pub(crate) thread_id: usize,

    /// Thread unpark handles
    #[cfg(feature = "sync")]
    pub(crate) unpark_cache:
        std::cell::RefCell<fxhash::FxHashMap<usize, crate::driver::UnparkHandle>>,

    /// Waker sender cache
    #[cfg(feature = "sync")]
    pub(crate) waker_sender_cache:
        std::cell::RefCell<fxhash::FxHashMap<usize, flume::Sender<std::task::Waker>>>,

    /// Owned task set and local run queue
    pub(crate) tasks: TaskQueue,
    /// Time Handle
    pub(crate) time_handle: Option<TimeHandle>,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub(crate) fn new_with_time_handle(time_handle: TimeHandle) -> Self {
        Self {
            time_handle: Some(time_handle),
            ..Self::new()
        }
    }

    pub(crate) fn new() -> Self {
        #[cfg(feature = "sync")]
        let thread_id = crate::builder::BUILD_THREAD_ID.with(|id| *id);

        Self {
            #[cfg(feature = "sync")]
            thread_id,
            #[cfg(feature = "sync")]
            unpark_cache: std::cell::RefCell::new(fxhash::FxHashMap::default()),
            #[cfg(feature = "sync")]
            waker_sender_cache: std::cell::RefCell::new(fxhash::FxHashMap::default()),
            tasks: TaskQueue::default(),
            time_handle: None,
        }
    }

    #[allow(unused)]
    #[cfg(feature = "sync")]
    pub(crate) fn unpark_thread(&self, id: usize) {
        use crate::driver::{thread::get_unpark_handle, Unpark};
        if let Some(handle) = self.unpark_cache.borrow().get(&id) {
            handle.unpark();
            return;
        }

        if let Some(v) = get_unpark_handle(id) {
            // Write back to local cache
            let w = v.clone();
            self.unpark_cache.borrow_mut().insert(id, w);
            v.unpark();
            return;
        }

        debug_assert!(false, "thread to unpark has not been registered");
    }

    #[allow(unused)]
    #[cfg(feature = "sync")]
    pub(crate) fn send_waker(&self, id: usize, w: std::task::Waker) {
        use crate::driver::thread::get_waker_sender;
        if let Some(sender) = self.waker_sender_cache.borrow().get(&id) {
            let _ = sender.send(w);
            return;
        }

        if let Some(s) = get_waker_sender(id) {
            // Write back to local cache
            let _ = s.send(w);
            self.waker_sender_cache.borrow_mut().insert(id, s);
            return;
        }

        debug_assert!(false, "sender has not been registered");
    }
}

/// Monoio runtime
pub struct Runtime<D> {
    pub(crate) driver: D,
    pub(crate) context: Context,
}

impl<D> Runtime<D> {
    /// Block on
    pub fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future,
        D: Driver,
    {
        assert!(
            !CURRENT.is_set(),
            "Can not start a runtime inside a runtime"
        );

        let waker = dummy_waker();
        let cx = &mut std::task::Context::from_waker(&waker);

        self.driver.with(|| {
            CURRENT.set(&self.context, || {
                #[cfg(feature = "sync")]
                let join = unsafe { spawn_without_static(future) };
                #[cfg(not(feature = "sync"))]
                let join = future;

                pin!(join);
                set_poll();
                loop {
                    loop {
                        // Consume all tasks(with max round to prevent io starvation)
                        let mut max_round = self.context.tasks.len() * 2;
                        while let Some(t) = self.context.tasks.pop() {
                            t.run();
                            if max_round == 0 {
                                // maybe there's a looping task
                                break;
                            } else {
                                max_round -= 1;
                            }
                        }

                        // Check main future
                        if should_poll() {
                            // check if ready
                            if let std::task::Poll::Ready(t) = join.as_mut().poll(cx) {
                                return t;
                            }
                        }

                        if self.context.tasks.is_empty() {
                            // No task to execute, we should wait for io blockingly
                            // Hot path
                            break;
                        }

                        // Cold path
                        let _ = self.driver.submit();
                    }

                    // Wait and Process CQ(the error is ignored for not debug mode)
                    #[cfg(not(all(debug_assertions, feature = "debug")))]
                    let _ = self.driver.park();

                    #[cfg(all(debug_assertions, feature = "debug"))]
                    if let Err(e) = self.driver.park() {
                        tracing!("park error: {:?}", e);
                    }
                }
            })
        })
    }
}

/// Spawns a new asynchronous task, returning a [`JoinHandle`] for it.
///
/// Spawning a task enables the task to execute concurrently to other tasks.
/// There is no guarantee that a spawned task will execute to completion. When a
/// runtime is shutdown, all outstanding tasks are dropped, regardless of the
/// lifecycle of that task.
///
///
/// [`JoinHandle`]: monoio::task::JoinHandle
///
/// # Examples
///
/// In this example, a server is started and `spawn` is used to start a new task
/// that processes each received connection.
///
/// ```no_run
/// monoio::start(async {
///     let handle = monoio::spawn(async {
///         println!("hello from a background task");
///     });
///
///     // Let the task complete
///     handle.await;
/// });
/// ```
pub fn spawn<T>(future: T) -> JoinHandle<T::Output>
where
    T: Future + 'static,
    T::Output: 'static,
{
    #[cfg(not(feature = "sync"))]
    let (task, join) = new_task(future, LocalScheduler);
    #[cfg(feature = "sync")]
    let (task, join) = new_task(
        crate::utils::thread_id::get_current_thread_id(),
        future,
        LocalScheduler,
    );

    CURRENT.with(|ctx| {
        ctx.tasks.push(task);
    });
    join
}

#[cfg(feature = "sync")]
unsafe fn spawn_without_static<T>(future: T) -> JoinHandle<T::Output>
where
    T: Future,
{
    use crate::task::new_task_holding;
    let (task, join) = new_task_holding(
        crate::utils::thread_id::get_current_thread_id(),
        future,
        LocalScheduler,
    );

    CURRENT.with(|ctx| {
        ctx.tasks.push(task);
    });
    join
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "sync")]
    #[test]
    fn across_thread() {
        use futures::channel::oneshot;
        let (tx1, rx1) = oneshot::channel::<u8>();
        let (tx2, rx2) = oneshot::channel::<u8>();

        std::thread::spawn(move || {
            let mut rt = crate::RuntimeBuilder::new().build().unwrap();
            rt.block_on(async move {
                let n = rx1.await.expect("unable to receive rx1");
                assert!(tx2.send(n).is_ok());
            });
        });

        let mut rt = crate::RuntimeBuilder::new().build().unwrap();
        rt.block_on(async move {
            assert!(tx1.send(24).is_ok());
            assert_eq!(rx2.await.expect("unable to receive rx2"), 24);
        });
    }

    #[test]
    fn timer() {
        let mut rt = crate::RuntimeBuilder::new().enable_timer().build().unwrap();
        let instant = std::time::Instant::now();
        rt.block_on(async {
            crate::time::sleep(std::time::Duration::from_millis(200)).await;
        });
        let eps = instant.elapsed().subsec_millis();
        assert!((eps as i32 - 200).abs() < 50);
    }
}
