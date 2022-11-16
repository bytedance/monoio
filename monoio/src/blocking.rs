//! Blocking tasks related.

use std::{future::Future, sync::Arc, task::Poll};

use threadpool::{Builder as ThreadPoolBuilder, ThreadPool as ThreadPoolImpl};

use crate::{
    task::{new_task, JoinHandle},
    utils::thread_id::DEFAULT_THREAD_ID,
};

/// Users may implement a ThreadPool and attach it to runtime.
/// We also provide an implementation based on threadpool crate, you can use DefaultThreadPool.
pub trait ThreadPool {
    /// Monoio runtime will call `schedule_task` on `spawn_blocking`.
    /// ThreadPool impl must execute it now or later.
    fn schedule_task(&self, task: BlockingTask);
}

/// Error on waiting blocking task.
#[derive(Debug, Clone, Copy)]
pub enum JoinError {
    /// Task is canceled.
    Canceled,
}

/// BlockingTask is contrusted by monoio, ThreadPool impl
/// will exeucte it with `.run()`.
pub struct BlockingTask {
    task: Option<crate::task::Task<NoopScheduler>>,
    blocking_vtable: &'static BlockingTaskVtable,
}

unsafe impl Send for BlockingTask {}

struct BlockingTaskVtable {
    pub(crate) drop: unsafe fn(&mut crate::task::Task<NoopScheduler>),
}

fn blocking_vtable<V>() -> &'static BlockingTaskVtable {
    &BlockingTaskVtable {
        drop: blocking_task_drop::<V>,
    }
}

fn blocking_task_drop<V>(task: &mut crate::task::Task<NoopScheduler>) {
    let mut opt: Option<Result<V, JoinError>> = Some(Err(JoinError::Canceled));
    unsafe { task.finish((&mut opt) as *mut _ as *mut ()) };
}

impl Drop for BlockingTask {
    fn drop(&mut self) {
        if let Some(task) = self.task.as_mut() {
            unsafe { (self.blocking_vtable.drop)(task) };
        }
    }
}

impl BlockingTask {
    /// Run task.
    #[inline]
    pub fn run(mut self) {
        let task = self.task.take().unwrap();
        task.run();
        // // if we are within a runtime, just run it.
        // if crate::runtime::CURRENT.is_set() {
        //     task.run();
        //     return;
        // }
        // // if we are on a standalone thread, we will use thread local ctx as Context.
        // crate::runtime::DEFAULT_CTX.with(|ctx| {
        //     crate::runtime::CURRENT.set(ctx, || task.run());
        // });
    }
}

/// BlockingStrategy can be set if there is no ThreadPool attached.
/// It controls how to handle `spawn_blocking` without thread pool.
#[derive(Clone, Copy, Debug)]
pub enum BlockingStrategy {
    /// Panic when `spawn_blocking`.
    Panic,
    /// Execute with current thread when `spawn_blocking`.
    ExecuteLocal,
}

/// `spawn_blocking` is used for executing a task(without async) with heavy computation or blocking
/// io. To used it, users may initialize a thread pool and attach it on creating runtime.
/// Users can also set `BlockingStrategy` for a runtime when there is no thread pool.
/// WARNING: DO NOT USE THIS FOR ASYNC TASK! Async tasks will not be executed but only built the
/// future!
pub fn spawn_blocking<F, R>(func: F) -> JoinHandle<Result<R, JoinError>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let fut = BlockingFuture(Some(func));
    let (task, join) = new_task(DEFAULT_THREAD_ID, fut, NoopScheduler);
    crate::runtime::CURRENT.with(|inner| {
        let handle = &inner.blocking_handle;
        match handle {
            BlockingHandle::Attached(shared) => shared.schedule_task(BlockingTask {
                task: Some(task),
                blocking_vtable: blocking_vtable::<R>(),
            }),
            BlockingHandle::Empty(BlockingStrategy::ExecuteLocal) => task.run(),
            BlockingHandle::Empty(BlockingStrategy::Panic) => {
                // For users: if you see this panic, you have 2 choices:
                // 1. attach a shared thread pool to execute blocking tasks
                // 2. set runtime blocking strategy to `BlockingStrategy::ExecuteLocal`
                // Note: solution 2 will execute blocking task on current thread and may block other
                // tasks This may cause other tasks high latency.
                panic!("execute blocking task without thread pool attached")
            }
        }
    });

    join
}

/// DefaultThreadPool is a simple wrapped `threadpool::ThreadPool` that implememt
/// `monoio::blocking::ThreadPool`. You may use this implementation, or you can use your own thread
/// pool implementation.
pub struct DefaultThreadPool {
    pool: ThreadPoolImpl,
}

impl DefaultThreadPool {
    /// Create a new DefaultThreadPool.
    pub fn new(num_threads: usize) -> Self {
        let pool = ThreadPoolBuilder::default()
            .num_threads(num_threads)
            .build();
        Self { pool }
    }
}

impl ThreadPool for DefaultThreadPool {
    fn schedule_task(&self, task: BlockingTask) {
        self.pool.execute(move || task.run());
    }
}

pub(crate) struct NoopScheduler;

impl crate::task::Schedule for NoopScheduler {
    fn schedule(&self, _task: crate::task::Task<Self>) {
        unreachable!()
    }

    fn yield_now(&self, _task: crate::task::Task<Self>) {
        unreachable!()
    }
}

#[derive(Clone)]
pub(crate) enum BlockingHandle {
    Attached(Arc<dyn ThreadPool>),
    Empty(BlockingStrategy),
}

impl From<BlockingStrategy> for BlockingHandle {
    fn from(value: BlockingStrategy) -> Self {
        Self::Empty(value)
    }
}

struct BlockingFuture<F>(Option<F>);

impl<T> Unpin for BlockingFuture<T> {}

impl<F, R> Future for BlockingFuture<F>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    type Output = Result<R, JoinError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let me = &mut *self;
        let func = me.0.take().expect("blocking task ran twice.");
        Poll::Ready(Ok(func()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::DefaultThreadPool;

    /// NaiveThreadPool always create a new thread on executing tasks.
    struct NaiveThreadPool;

    impl super::ThreadPool for NaiveThreadPool {
        fn schedule_task(&self, task: super::BlockingTask) {
            std::thread::spawn(move || {
                task.run();
            });
        }
    }

    /// FakeThreadPool always drop tasks.
    struct FakeThreadPool;

    impl super::ThreadPool for FakeThreadPool {
        fn schedule_task(&self, _task: super::BlockingTask) {}
    }

    #[test]
    fn hello_blocking() {
        let shared_pool = Arc::new(NaiveThreadPool);
        let mut rt = crate::RuntimeBuilder::<crate::FusionDriver>::new()
            .attach_thread_pool(shared_pool)
            .enable_timer()
            .build()
            .unwrap();
        rt.block_on(async {
            let begin = std::time::Instant::now();
            let join = crate::spawn_blocking(|| {
                // Simulate a heavy computation.
                std::thread::sleep(std::time::Duration::from_millis(400));
                "hello spawn_blocking!".to_string()
            });
            let sleep_async = crate::time::sleep(std::time::Duration::from_millis(400));
            let (result, _) = crate::join!(join, sleep_async);
            let eps = begin.elapsed();
            assert!(eps < std::time::Duration::from_millis(800));
            assert!(eps >= std::time::Duration::from_millis(400));
            assert_eq!(result.unwrap(), "hello spawn_blocking!");
        });
    }

    #[test]
    #[should_panic]
    fn blocking_panic() {
        let mut rt = crate::RuntimeBuilder::<crate::FusionDriver>::new()
            .with_blocking_strategy(crate::blocking::BlockingStrategy::Panic)
            .enable_timer()
            .build()
            .unwrap();
        rt.block_on(async {
            let join = crate::spawn_blocking(|| 1);
            let _ = join.await;
        });
    }

    #[test]
    fn blocking_current() {
        let mut rt = crate::RuntimeBuilder::<crate::FusionDriver>::new()
            .with_blocking_strategy(crate::blocking::BlockingStrategy::ExecuteLocal)
            .enable_timer()
            .build()
            .unwrap();
        rt.block_on(async {
            let begin = std::time::Instant::now();
            let join = crate::spawn_blocking(|| {
                // Simulate a heavy computation.
                std::thread::sleep(std::time::Duration::from_millis(100));
                "hello spawn_blocking!".to_string()
            });
            let sleep_async = crate::time::sleep(std::time::Duration::from_millis(100));
            let (result, _) = crate::join!(join, sleep_async);
            let eps = begin.elapsed();
            assert!(eps > std::time::Duration::from_millis(200));
            assert_eq!(result.unwrap(), "hello spawn_blocking!");
        });
    }

    #[test]
    fn drop_task() {
        let shared_pool = Arc::new(FakeThreadPool);
        let mut rt = crate::RuntimeBuilder::<crate::FusionDriver>::new()
            .attach_thread_pool(shared_pool)
            .enable_timer()
            .build()
            .unwrap();
        rt.block_on(async {
            let ret = crate::spawn_blocking(|| 1).await;
            assert!(matches!(ret, Err(super::JoinError::Canceled)));
        });
    }

    #[test]
    fn default_pool() {
        let shared_pool = Arc::new(DefaultThreadPool::new(3));
        let mut rt = crate::RuntimeBuilder::<crate::FusionDriver>::new()
            .attach_thread_pool(shared_pool)
            .enable_timer()
            .build()
            .unwrap();
        rt.block_on(async {
            let begin = std::time::Instant::now();
            let join1 = crate::spawn_blocking(|| {
                // Simulate a heavy computation.
                std::thread::sleep(std::time::Duration::from_millis(150));
                "hello spawn_blocking1!".to_string()
            });
            let join2 = crate::spawn_blocking(|| {
                // Simulate a heavy computation.
                std::thread::sleep(std::time::Duration::from_millis(150));
                "hello spawn_blocking2!".to_string()
            });
            let join3 = crate::spawn_blocking(|| {
                // Simulate a heavy computation.
                std::thread::sleep(std::time::Duration::from_millis(150));
                "hello spawn_blocking3!".to_string()
            });
            let join4 = crate::spawn_blocking(|| {
                // Simulate a heavy computation.
                std::thread::sleep(std::time::Duration::from_millis(150));
                "hello spawn_blocking4!".to_string()
            });
            let sleep_async = crate::time::sleep(std::time::Duration::from_millis(150));
            let (result1, result2, result3, result4, _) =
                crate::join!(join1, join2, join3, join4, sleep_async);
            let eps = begin.elapsed();
            assert!(eps < std::time::Duration::from_millis(590));
            assert!(eps >= std::time::Duration::from_millis(150));
            assert_eq!(result1.unwrap(), "hello spawn_blocking1!");
            assert_eq!(result2.unwrap(), "hello spawn_blocking2!");
            assert_eq!(result3.unwrap(), "hello spawn_blocking3!");
            assert_eq!(result4.unwrap(), "hello spawn_blocking4!");
        });
    }
}
