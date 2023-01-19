use std::{
    time, time::Duration,
    ops::Deref,
    sync::{Arc, Condvar, Mutex, MutexGuard}
};

// ------------------------------ DATA STRUCTURE ------------------------------
#[derive(Debug)]
pub enum WaitObjectError {
    /// Meaning a sync object gets broken (or poisoned) due to panic!()
    SynchronizationBroken
}

pub type Result<T> = std::result::Result<T, WaitObjectError>;

/// Create a wait event object of any type T. To use this wait object in multi-threaded scenario, just clone the object and distribute it.
///
/// This wait object is just a wrapper of Mutex and Condvar combination with the suggested pattern (from Rust document) for waiting a value.
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
/// let r#final = *wait3.wait(|i| *i == 3, None).unwrap();
/// assert_eq!(r#final, 3);
/// ```
#[derive(Clone)]
pub struct WaitEvent<T>(Arc<(Mutex<T>, Condvar)>);

#[derive(Clone)]
pub struct ManualResetEvent(WaitEvent<bool>);

#[derive(Clone)]
pub struct AutoResetEvent(WaitEvent<bool>);

// ------------------------------ IMPLEMENTATIONS ------------------------------
impl<T> WaitEvent<T> {
    #[inline]
    pub fn new_init(initial_state: T) -> Self {
        Self(Arc::new((Mutex::new(initial_state), Condvar::new())))
    }

    #[inline]
    pub fn wait(&self, checker: impl FnMut(&T) -> bool, timeout: Option<Duration>) -> Result<MutexGuard<T>> {
        match timeout {
            Some(t) => self.wait_with_waiter(checker, Self::waiter(t)),
            None => self.wait_with_waiter(checker, Self::no_waiter())
        }
    }

    fn wait_with_waiter(&self, mut checker: impl FnMut(&T) -> bool, waiter: impl Fn() -> bool) -> Result<MutexGuard<T>> {
        let (lock, cond) = self.0.deref();
        let mut state = lock.lock()?;
        while waiter() && !checker(&*state) {
            state = cond.wait(state)?;
        }
        Ok(state)
    }

    fn waiter(timeout: Duration) -> impl Fn() -> bool {
        let start = time::Instant::now();
        move || { (time::Instant::now() - start) < timeout }
    }
    fn no_waiter() -> impl Fn() -> bool { || true }

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

    #[inline]
    pub fn wait_until_set(&self) -> Result<bool> {
        self.0.wait(|v| *v, None).map(|g| *g)
    }

    #[inline] pub fn wait_one(&self, timeout: Duration) -> Result<bool> {
        self.0.wait(|v| *v, Some(timeout)).map(|g| *g)
    }

    #[inline]
    pub fn reset(&mut self) -> Result<()> {
        self.0.set_state(false)
    }

    #[inline]
    pub fn set(&mut self) -> Result<()> {
        self.0.set_state(true)
    }
}

impl<T> From<std::sync::PoisonError<T>> for WaitObjectError {
    fn from(_value: std::sync::PoisonError<T>) -> Self {
        Self::SynchronizationBroken
    }
}
