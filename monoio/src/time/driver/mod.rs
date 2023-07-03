// Currently, rust warns when an unsafe fn contains an unsafe {} block. However,
// in the future, this will change to the reverse. For now, suppress this
// warning and generally stick with being explicit about unsafety.
#![allow(unused_unsafe)]
#![cfg_attr(not(feature = "rt"), allow(dead_code))]

//! Time driver

mod entry;
use self::entry::{EntryList, TimerEntry, TimerHandle, TimerShared};

mod handle;
pub(crate) use self::handle::Handle;

mod wheel;

pub(super) mod sleep;

use std::{cell::RefCell, convert::TryInto, fmt, io, num::NonZeroU64, ptr::NonNull, rc::Rc};

use crate::{
    driver::Driver,
    time::{error::Error, Clock, Duration, Instant},
};

/// Time implementation that drives [`Sleep`][sleep], [`Interval`][interval],
/// and [`Timeout`][timeout].
///
/// A `Driver` instance tracks the state necessary for managing time and
/// notifying the [`Sleep`][sleep] instances once their deadlines are reached.
///
/// It is expected that a single instance manages many individual
/// [`Sleep`][sleep] instances. The `Driver` implementation is thread-safe and,
/// as such, is able to handle callers from across threads.
///
/// After creating the `Driver` instance, the caller must repeatedly call `park`
/// or `park_timeout`. The time driver will perform no work unless `park` or
/// `park_timeout` is called repeatedly.
///
/// The driver has a resolution of one millisecond. Any unit of time that falls
/// between milliseconds are rounded up to the next millisecond.
///
/// When an instance is dropped, any outstanding [`Sleep`][sleep] instance that
/// has not elapsed will be notified with an error. At this point, calling
/// `poll` on the [`Sleep`][sleep] instance will result in panic.
///
/// # Implementation
///
/// The time driver is based on the [paper by Varghese and Lauck][paper].
///
/// A hashed timing wheel is a vector of slots, where each slot handles a time
/// slice. As time progresses, the timer walks over the slot for the current
/// instant, and processes each entry for that slot. When the timer reaches the
/// end of the wheel, it starts again at the beginning.
///
/// The implementation maintains six wheels arranged in a set of levels. As the
/// levels go up, the slots of the associated wheel represent larger intervals
/// of time. At each level, the wheel has 64 slots. Each slot covers a range of
/// time equal to the wheel at the lower level. At level zero, each slot
/// represents one millisecond of time.
///
/// The wheels are:
///
/// * Level 0: 64 x 1 millisecond slots.
/// * Level 1: 64 x 64 millisecond slots.
/// * Level 2: 64 x ~4 second slots.
/// * Level 3: 64 x ~4 minute slots.
/// * Level 4: 64 x ~4 hour slots.
/// * Level 5: 64 x ~12 day slots.
///
/// When the timer processes entries at level zero, it will notify all the
/// `Sleep` instances as their deadlines have been reached. For all higher
/// levels, all entries will be redistributed across the wheel at the next level
/// down. Eventually, as time progresses, entries with [`Sleep`][sleep]
/// instances will either be canceled (dropped) or their associated entries will
/// reach level zero and be notified.
///
/// [paper]: http://www.cs.columbia.edu/~nahum/w6998/papers/ton97-timing-wheels.pdf
/// [sleep]: crate::time::Sleep
/// [timeout]: crate::time::Timeout
/// [interval]: crate::time::Interval
#[derive(Debug)]
pub struct TimeDriver<D: 'static> {
    /// Timing backend in use
    time_source: ClockTime,

    /// Shared state
    pub(crate) handle: Handle,

    /// Parker to delegate to
    park: D,
}

/// A structure which handles conversion from Instants to u64 timestamps.
#[derive(Debug, Clone)]
struct ClockTime {
    clock: super::clock::Clock,
    start_time: Instant,
}

impl ClockTime {
    pub(self) fn new(clock: Clock) -> Self {
        Self {
            start_time: clock.now(),
            clock,
        }
    }

    pub(self) fn deadline_to_tick(&self, t: Instant) -> u64 {
        // Round up to the end of a ms
        self.instant_to_tick(t + Duration::from_nanos(999_999))
    }

    pub(self) fn instant_to_tick(&self, t: Instant) -> u64 {
        // round up
        let dur: Duration = t
            .checked_duration_since(self.start_time)
            .unwrap_or_else(|| Duration::from_secs(0));
        let ms = dur.as_millis();

        ms.try_into().expect("Duration too far into the future")
    }

    pub(self) fn tick_to_duration(&self, t: u64) -> Duration {
        Duration::from_millis(t)
    }

    pub(self) fn now(&self) -> u64 {
        self.instant_to_tick(self.clock.now())
    }
}

/// Timer state shared between `Driver`, `Handle`, and `Registration`.
struct Inner {
    // The state is split like this so `Handle` can access `is_shutdown` without locking the mutex
    pub(super) state: RefCell<InnerState>,
}

/// Time state shared which must be protected by a `Mutex`
struct InnerState {
    /// Timing backend in use
    time_source: ClockTime,

    /// The last published timer `elapsed` value.
    elapsed: u64,

    /// The earliest time at which we promise to wake up without unparking
    next_wake: Option<NonZeroU64>,

    /// Timer wheel
    wheel: wheel::Wheel,
}

// ===== impl Driver =====

impl<D> TimeDriver<D>
where
    D: Driver + 'static,
{
    /// Creates a new `Driver` instance that uses `park` to block the current
    /// thread and `time_source` to get the current time and convert to ticks.
    ///
    /// Specifying the source of time is useful when testing.
    pub(crate) fn new(park: D, clock: Clock) -> TimeDriver<D> {
        let time_source = ClockTime::new(clock);

        let inner = Inner::new(time_source.clone());

        TimeDriver {
            time_source,
            handle: Handle::new(Rc::new(inner)),
            park,
        }
    }

    /// Returns a handle to the timer.
    ///
    /// The `Handle` is how `Sleep` instances are created. The `Sleep` instances
    /// can either be created directly or the `Handle` instance can be passed to
    /// `with_default`, setting the timer as the default timer for the execution
    /// context.
    pub(crate) fn handle(&self) -> Handle {
        self.handle.clone()
    }

    fn park_internal(&self, limit: Option<Duration>) -> io::Result<()> {
        let mut inner_state = self.handle.get().state.borrow_mut();

        let next_wake = inner_state.wheel.next_expiration_time();
        inner_state.next_wake =
            next_wake.map(|t| NonZeroU64::new(t).unwrap_or_else(|| NonZeroU64::new(1).unwrap()));
        drop(inner_state);

        match next_wake {
            Some(when) => {
                let now = self.time_source.now();
                // Note that we effectively round up to 1ms here - this avoids
                // very short-duration microsecond-resolution sleeps that the OS
                // might treat as zero-length.
                let mut duration = self.time_source.tick_to_duration(when.saturating_sub(now));

                if duration > Duration::from_millis(0) {
                    if let Some(limit) = limit {
                        duration = std::cmp::min(limit, duration);
                    }

                    self.park.park_timeout(duration)?;
                } else {
                    self.park.park_timeout(Duration::from_secs(0))?;
                }
            }
            None => {
                if let Some(duration) = limit {
                    self.park.park_timeout(duration)?;
                } else {
                    self.park.park()?;
                }
            }
        }

        // Process pending timers after waking up
        self.handle.process();

        Ok(())
    }
}

impl Handle {
    /// Runs timer related logic, and returns the next wakeup time
    pub(self) fn process(&self) {
        let now = self.time_source().now();

        self.process_at_time(now)
    }

    pub(self) fn process_at_time(&self, mut now: u64) {
        let mut state = self.get().state.borrow_mut();

        if now < state.elapsed {
            // Time went backwards! This normally shouldn't happen as the Rust language
            // guarantees that an Instant is monotonic, but can happen when running
            // Linux in a VM on a Windows host due to std incorrectly trusting the
            // hardware clock to be monotonic.
            //
            // See <https://github.com/tokio-rs/tokio/issues/3619> for more information.
            now = state.elapsed;
        }
        while let Some(entry) = state.wheel.poll(now) {
            if let Some(waker) = unsafe { entry.fire(Ok(())) } {
                waker.wake();
            }
        }
        state.elapsed = state.wheel.elapsed();
        state.next_wake = state
            .wheel
            .poll_at()
            .map(|t| NonZeroU64::new(t).unwrap_or_else(|| NonZeroU64::new(1).unwrap()));
    }

    /// Removes a registered timer from the driver.
    ///
    /// The timer will be moved to the cancelled state. Wakers will _not_ be
    /// invoked. If the timer is already completed, this function is a no-op.
    ///
    /// This function always acquires the driver lock, even if the entry does
    /// not appear to be registered.
    ///
    /// SAFETY: The timer must not be registered with some other driver, and
    /// `add_entry` must not be called concurrently.
    pub(self) unsafe fn clear_entry(&self, entry: NonNull<TimerShared>) {
        unsafe {
            let mut state = self.get().state.borrow_mut();
            if entry.as_ref().might_be_registered() {
                state.wheel.remove(entry);
            }

            entry.as_ref().handle().fire(Ok(()));
        }
    }

    /// Removes and re-adds an entry to the driver.
    ///
    /// SAFETY: The timer must be either unregistered, or registered with this
    /// driver. No other threads are allowed to concurrently manipulate the
    /// timer at all (the current thread should hold an exclusive reference to
    /// the `TimerEntry`)
    pub(self) unsafe fn reregister(&self, new_tick: u64, entry: NonNull<TimerShared>) {
        let waker = unsafe {
            let mut state = self.get().state.borrow_mut();

            // We may have raced with a firing/deregistration, so check before
            // deregistering.
            if unsafe { entry.as_ref().might_be_registered() } {
                state.wheel.remove(entry);
            }

            // Now that we have exclusive control of this entry, mint a handle to reinsert
            // it.
            let entry = entry.as_ref().handle();

            entry.set_expiration(new_tick);

            // Note: We don't have to worry about racing with some other resetting
            // thread, because add_entry and reregister require exclusive control of
            // the timer entry.
            match unsafe { state.wheel.insert(entry) } {
                Ok(_) => None,
                Err((entry, super::error::InsertError::Elapsed)) => unsafe { entry.fire(Ok(())) },
            }
        };

        // The timer was fired synchronously as a result of the reregistration.
        // Wake the waker; this is needed because we might reset _after_ a poll,
        // and otherwise the task won't be awoken to poll again.
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<D> Driver for TimeDriver<D>
where
    D: Driver + 'static,
{
    fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        self.park.with(f)
    }

    fn submit(&self) -> io::Result<()> {
        self.park.submit()
    }

    fn park(&self) -> io::Result<()> {
        self.park_internal(None)
    }

    #[cfg(feature = "sync")]
    type Unpark = D::Unpark;

    fn park_timeout(&self, duration: Duration) -> io::Result<()> {
        self.park_internal(Some(duration))
    }

    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark {
        self.park.unpark()
    }
}

impl<D> Drop for TimeDriver<D>
where
    D: 'static,
{
    fn drop(&mut self) {
        // self.shutdown();
    }
}

// ===== impl Inner =====

impl Inner {
    pub(self) fn new(time_source: ClockTime) -> Self {
        Inner {
            state: RefCell::new(InnerState {
                time_source,
                elapsed: 0,
                next_wake: None,
                wheel: wheel::Wheel::new(),
            }),
        }
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Inner").finish()
    }
}
