use std::{time, time::Duration, ops::Deref, sync::{Arc, Condvar, Mutex, MutexGuard}, mem};
use std::ops::DerefMut;

#[cfg(windows)]
pub mod windows;

// ------------------------------ DATA TYPES ----------------------------------
#[derive(Debug)]
pub enum WaitObjectError {
    /// OS error code with its description. This error code is only when using APIs based on OS.
    OsError(isize, String),

    /// Meaning a sync object gets broken (or poisoned) due to panic!()
    SynchronizationBroken
}

pub type Result<T> = std::result::Result<T, WaitObjectError>;

/// Create a wait event object of any type T. To use this wait object in multi-threaded scenario, just clone the object and distribute it.
///
/// This wait object is just a wrapper of Mutex and Condvar combination with the suggested pattern (from Rust document) for waiting a value.
///
/// There are two ways to wait. The first is just to want until an expected value.
///
/// ```rust
/// # use sync_wait_object::WaitEvent;
/// use std::thread;
/// let wait3 = WaitEvent::new_init(0);
/// let mut wait_handle = wait3.clone();
///
/// thread::spawn(move || {
///     for i in 1..=3 {
///         wait_handle.set_state(i).unwrap();
///     }
/// });
///
/// let timeout = std::time::Duration::from_secs(1);
/// let r#final = *wait3.wait(Some(timeout), |i| *i == 3).unwrap();
/// let current = *wait3.value().unwrap();
/// assert_eq!(r#final, 3);
/// assert_eq!(current, 3);
/// ```
///
/// The second is to wait and then reset the value to a desired state.
/// ```rust
/// # use sync_wait_object::WaitEvent;
/// use std::thread;
/// let wait3 = WaitEvent::new_init(0);
/// let mut wait_handle = wait3.clone();
///
/// thread::spawn(move || {
///     for i in 1..=3 {
///         wait_handle.set_state(i).unwrap();
///     }
/// });
///
/// let timeout = std::time::Duration::from_secs(1);
/// let r#final = wait3.wait_reset(Some(timeout), || 1, |i| *i == 3).unwrap();
/// let current = *wait3.value().unwrap();
/// assert_eq!(r#final, 3);
/// assert_eq!(current, 1);
/// ```
///
#[derive(Clone)]
pub struct WaitEvent<T>(Arc<(Mutex<T>, Condvar)>);

/// Wrapper of [`WaitEvent`] of type `bool`, which focuses on waiting for `true` without resetting.
#[derive(Clone)]
pub struct ManualResetEvent(WaitEvent<bool>);

/// Wrapper of [`WaitEvent`] of type `bool`, which focuses on waiting for `true` with automatic reset to `false`.
#[derive(Clone)]
pub struct AutoResetEvent(WaitEvent<bool>);

// Boolean signal with ability to wait and set state.
pub trait SignalWaitable {
    fn wait_until_set(&self) -> Result<bool>;
    fn wait(&self, timeout: Duration) -> Result<bool>;
    fn set(&mut self) -> Result<()>;
    fn reset(&mut self) -> Result<()>;
}

// ------------------------------ IMPLEMENTATIONS ------------------------------
impl<T> WaitEvent<T> {
    #[inline]
    pub fn new_init(initial_state: T) -> Self {
        Self(Arc::new((Mutex::new(initial_state), Condvar::new())))
    }

    pub fn value(&self) -> Result<MutexGuard<T>> {
        self.0.0.lock().map_err(|e| e.into())
    }

    pub fn wait(&self, timeout: Option<Duration>, checker: impl FnMut(&T) -> bool) -> Result<MutexGuard<T>> {
        match timeout {
            Some(t) => self.wait_with_waiter(checker, Self::waiter(t)),
            None => self.wait_with_waiter(checker, Self::no_waiter())
        }
    }

    pub fn wait_reset(&self, timeout: Option<Duration>, reset: impl FnMut() -> T, checker: impl FnMut(&T) -> bool) -> Result<T> {
        match timeout {
            Some(t) => self.wait_and_reset_with_waiter(checker, Self::waiter(t), reset),
            None => self.wait_and_reset_with_waiter(checker, Self::no_waiter(), reset)
        }
    }

    pub fn wait_with_waiter(&self, mut checker: impl FnMut(&T) -> bool, waiter: impl Fn() -> bool) -> Result<MutexGuard<T>> {
        let (lock, cond) = self.0.deref();
        let mut state = lock.lock()?;
        while waiter() && !checker(&*state) {
            state = cond.wait(state)?;
        }
        Ok(state)
    }

    pub fn wait_and_reset_with_waiter(&self, checker: impl FnMut(&T) -> bool, waiter: impl Fn() -> bool, mut reset: impl FnMut() -> T) -> Result<T> {
        let state = self.wait_with_waiter(checker, waiter);
        state.map(|mut g| mem::replace(g.deref_mut(), reset()))
    }

    pub fn waiter(timeout: Duration) -> impl Fn() -> bool {
        let start = time::Instant::now();
        move || { (time::Instant::now() - start) < timeout }
    }

    #[inline]
    pub fn no_waiter() -> impl Fn() -> bool { || true }

    pub fn set_state(&mut self, new_state: T) -> Result<()> {
        let (lock, cond) = self.0.deref();
        let mut state = lock.lock()?;
        *state = new_state;
        cond.notify_all();
        Ok(())
    }
}

impl ManualResetEvent {
    #[inline]
    pub fn new() -> Self { Self::new_init(false) }

    #[inline]
    pub fn new_init(initial_state: bool) -> Self {
        Self(WaitEvent::new_init(initial_state))
    }
}

impl SignalWaitable for ManualResetEvent {
    #[inline]
    fn wait_until_set(&self) -> Result<bool> {
        self.0.wait(None, |v| *v).map(|g| *g)
    }

    #[inline] fn wait(&self, timeout: Duration) -> Result<bool> {
        self.0.wait(Some(timeout), |v| *v).map(|g| *g)
    }

    #[inline]
    fn set(&mut self) -> Result<()> {
        self.0.set_state(true)
    }

    #[inline]
    fn reset(&mut self) -> Result<()> {
        self.0.set_state(false)
    }
}

impl AutoResetEvent {
    #[inline]
    pub fn new() -> Self { Self::new_init(false) }

    #[inline]
    fn new_init(initial_state: bool) -> Self {
        Self(WaitEvent::new_init(initial_state))
    }
}

impl SignalWaitable for AutoResetEvent {
    #[inline]
    fn wait_until_set(&self) -> Result<bool> {
        self.0.wait_reset(None, || false, |v| *v)
    }

    #[inline] fn wait(&self, timeout: Duration) -> Result<bool> {
        self.0.wait_reset(Some(timeout), || false, |v| *v)
    }

    #[inline]
    fn set(&mut self) -> Result<()> {
        self.0.set_state(true)
    }

    #[inline]
    fn reset(&mut self) -> Result<()> {
        self.0.set_state(false)
    }
}

impl<T> From<std::sync::PoisonError<T>> for WaitObjectError {
    fn from(_value: std::sync::PoisonError<T>) -> Self {
        Self::SynchronizationBroken
    }
}

impl From<WaitEvent<bool>> for ManualResetEvent {
    fn from(value: WaitEvent<bool>) -> Self {
                                          Self(value)
                                                     }
}

impl From<ManualResetEvent> for WaitEvent<bool> {
    fn from(value: ManualResetEvent) -> Self {
                                           value.0
                                                  }
}

impl From<WaitEvent<bool>> for AutoResetEvent {
    fn from(value: WaitEvent<bool>) -> Self {
                                          Self(value)
                                                     }
}

impl From<AutoResetEvent> for WaitEvent<bool> {
    fn from(value: AutoResetEvent) -> Self {
                                         value.0
                                                }
}