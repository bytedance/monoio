use std::{
    future::Future,
    pin::Pin,
    task::{self, Poll},
};

use pin_project_lite::pin_project;

use crate::time::{
    driver::{Handle, TimerEntry},
    error::Error,
    Duration, Instant,
};

/// Waits until `deadline` is reached.
///
/// No work is performed while awaiting on the sleep future to complete. `Sleep`
/// operates at millisecond granularity and should not be used for tasks that
/// require high-resolution timers.
///
/// To run something regularly on a schedule, see [`interval`].
///
/// # Cancellation
///
/// Canceling a sleep instance is done by dropping the returned future. No
/// additional cleanup work is required.
///
/// # Examples
///
/// Wait 100ms and print "100 ms have elapsed".
///
/// ```
/// use monoio::time::{sleep_until, Duration, Instant};
///
/// #[monoio::main(timer_enabled = true)]
/// async fn main() {
///     sleep_until(Instant::now() + Duration::from_millis(100)).await;
///     println!("100 ms have elapsed");
/// }
/// ```
///
/// See the documentation for the [`Sleep`] type for more examples.
///
/// [`Sleep`]: struct@crate::time::Sleep
/// [`interval`]: crate::time::interval()
// Alias for old name in 0.x
#[cfg_attr(docsrs, doc(alias = "delay_until"))]
pub fn sleep_until(deadline: Instant) -> Sleep {
    Sleep::new_timeout(deadline)
}

/// Waits until `duration` has elapsed.
///
/// Equivalent to `sleep_until(Instant::now() + duration)`. An asynchronous
/// analog to `std::thread::sleep`.
///
/// No work is performed while awaiting on the sleep future to complete. `Sleep`
/// operates at millisecond granularity and should not be used for tasks that
/// require high-resolution timers.
///
/// To run something regularly on a schedule, see [`interval`].
///
/// The maximum duration for a sleep is 68719476734 milliseconds (approximately
/// 2.2 years).
///
/// # Cancellation
///
/// Canceling a sleep instance is done by dropping the returned future. No
/// additional cleanup work is required.
///
/// # Examples
///
/// Wait 100ms and print "100 ms have elapsed".
///
/// ```
/// use monoio::time::{sleep, Duration};
///
/// #[monoio::main(timer_enabled = true)]
/// async fn main() {
///     sleep(Duration::from_millis(100)).await;
///     println!("100 ms have elapsed");
/// }
/// ```
///
/// See the documentation for the [`Sleep`] type for more examples.
///
/// [`Sleep`]: struct@crate::time::Sleep
/// [`interval`]: crate::time::interval()
// Alias for old name in 0.x
#[cfg_attr(docsrs, doc(alias = "delay_for"))]
#[cfg_attr(docsrs, doc(alias = "wait"))]
pub fn sleep(duration: Duration) -> Sleep {
    match Instant::now().checked_add(duration) {
        Some(deadline) => sleep_until(deadline),
        None => sleep_until(Instant::far_future()),
    }
}

pin_project! {
    /// Future returned by [`sleep`](sleep) and [`sleep_until`](sleep_until).
    ///
    /// This type does not implement the `Unpin` trait, which means that if you
    /// use it with [`select!`] or by calling `poll`, you have to pin it first.
    /// If you use it with `.await`, this does not apply.
    ///
    /// # Examples
    ///
    /// Wait 100ms and print "100 ms have elapsed".
    ///
    /// ```
    /// use monoio::time::{sleep, Duration};
    ///
    /// #[monoio::main(timer_enabled = true)]
    /// async fn main() {
    ///     sleep(Duration::from_millis(100)).await;
    ///     println!("100 ms have elapsed");
    /// }
    /// ```
    ///
    /// Use with [`select!`]. Pinning the `Sleep` with [`monoio::pin!`] is
    /// necessary when the same `Sleep` is selected on multiple times.
    /// ```no_run
    /// use monoio::time::{self, Duration, Instant};
    ///
    /// #[monoio::main(timer_enabled = true)]
    /// async fn main() {
    ///     let sleep = time::sleep(Duration::from_millis(10));
    ///     monoio::pin!(sleep);
    ///
    ///     loop {
    ///         monoio::select! {
    ///             () = &mut sleep => {
    ///                 println!("timer elapsed");
    ///                 sleep.as_mut().reset(Instant::now() + Duration::from_millis(50));
    ///             },
    ///         }
    ///     }
    /// }
    /// ```
    /// Use in a struct with boxing. By pinning the `Sleep` with a `Box`, the
    /// `HasSleep` struct implements `Unpin`, even though `Sleep` does not.
    /// ```
    /// use std::future::Future;
    /// use std::pin::Pin;
    /// use std::task::{Context, Poll};
    /// use monoio::time::Sleep;
    ///
    /// struct HasSleep {
    ///     sleep: Pin<Box<Sleep>>,
    /// }
    ///
    /// impl Future for HasSleep {
    ///     type Output = ();
    ///
    ///     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    ///         self.sleep.as_mut().poll(cx)
    ///     }
    /// }
    /// ```
    /// Use in a struct with pin projection. This method avoids the `Box`, but
    /// the `HasSleep` struct will not be `Unpin` as a consequence.
    /// ```
    /// use std::future::Future;
    /// use std::pin::Pin;
    /// use std::task::{Context, Poll};
    /// use monoio::time::Sleep;
    /// use pin_project_lite::pin_project;
    ///
    /// pin_project! {
    ///     struct HasSleep {
    ///         #[pin]
    ///         sleep: Sleep,
    ///     }
    /// }
    ///
    /// impl Future for HasSleep {
    ///     type Output = ();
    ///
    ///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    ///         self.project().sleep.poll(cx)
    ///     }
    /// }
    /// ```
    ///
    /// [`select!`]: ../macro.select.html
    /// [`monoio::pin!`]: ../macro.pin.html
    // Alias for old name in 0.2
    #[cfg_attr(docsrs, doc(alias = "Delay"))]
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Sleep {
        deadline: Instant,

        // The link between the `Sleep` instance and the timer that drives it.
        #[pin]
        entry: TimerEntry,
    }
}

impl Sleep {
    pub(crate) fn new_timeout(deadline: Instant) -> Sleep {
        let handle = Handle::current();
        let entry = TimerEntry::new(&handle, deadline);

        Sleep { deadline, entry }
    }

    pub(crate) fn far_future() -> Sleep {
        Self::new_timeout(Instant::far_future())
    }

    /// Returns the instant at which the future will complete.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Returns `true` if `Sleep` has elapsed.
    ///
    /// A `Sleep` instance is elapsed when the requested duration has elapsed.
    pub fn is_elapsed(&self) -> bool {
        self.entry.is_elapsed()
    }

    /// Resets the `Sleep` instance to a new deadline.
    ///
    /// Calling this function allows changing the instant at which the `Sleep`
    /// future completes without having to create new associated state.
    ///
    /// This function can be called both before and after the future has
    /// completed.
    ///
    /// To call this method, you will usually combine the call with
    /// [`Pin::as_mut`], which lets you call the method without consuming the
    /// `Sleep` itself.
    ///
    /// # Example
    ///
    /// ```
    /// use monoio::time::{Duration, Instant};
    ///
    /// # #[monoio::main(timer_enabled = true)]
    /// # async fn main() {
    /// let sleep = monoio::time::sleep(Duration::from_millis(10));
    /// monoio::pin!(sleep);
    ///
    /// sleep
    ///     .as_mut()
    ///     .reset(Instant::now() + Duration::from_millis(20));
    /// # }
    /// ```
    ///
    /// See also the top-level examples.
    ///
    /// [`Pin::as_mut`]: fn@std::pin::Pin::as_mut
    pub fn reset(self: Pin<&mut Self>, deadline: Instant) {
        let me = self.project();
        me.entry.reset(deadline);
        *me.deadline = deadline;
    }

    fn poll_elapsed(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Result<(), Error>> {
        let me = self.project();
        me.entry.poll_elapsed(cx)
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        // `poll_elapsed` can return an error in two cases:
        //
        // - AtCapacity: this is a pathological case where far too many sleep instances have been
        //   scheduled.
        // - Shutdown: No timer has been setup, which is a mis-use error.
        //
        // Both cases are extremely rare, and pretty accurately fit into
        // "logic errors", so we just panic in this case. A user couldn't
        // really do much better if we passed the error onwards.
        match ready!(self.as_mut().poll_elapsed(cx)) {
            Ok(()) => Poll::Ready(()),
            Err(e) => panic!("timer error: {e}"),
        }
    }
}
