///! Windows implementation of `ManualResetEvent` and `AutoResetEvent` which directly wraps over Win32 API.

use std::{
    time::Duration,
    ops::{ Deref, DerefMut }
};
use windows::Win32::{
    Foundation::{ HANDLE, CloseHandle, GetLastError, WAIT_OBJECT_0, WAIT_TIMEOUT, WAIT_FAILED, WIN32_ERROR },
    System::Threading::{ CreateEventA, WaitForSingleObject, ResetEvent, SetEvent },
    System::WindowsProgramming::INFINITE
};
use crate::{ WaitObjectError, Result, SignalWaitable };

// --------------------------------------- DATA STRUCTURE ---------------------------------------------
#[derive(Clone)]
pub struct WaitEvent(HANDLE);

/// Wrapper of [`WaitEvent`] of type `bool`, which focuses on waiting for `true` without resetting.
///
/// *Examples*
/// The example does not make sense (as [`sync_wait_object::WaitEvent`] of `u8` makes more sense for this scenario), it is just for demonstration.
///
/// ```rust
/// # use sync_wait_object::windows::ManualResetEvent;
/// use std::{ thread, sync::Arc, sync::atomic::{ AtomicU8, Ordering } };
/// use sync_wait_object::SignalWaitable;
///
/// let ev = ManualResetEvent::new();
/// let mut signal = ev.clone();
/// let v = Arc::new(AtomicU8::new(0));
/// let v_setter = v.clone();
///
/// thread::spawn(move || {
///     v_setter.store(1, Ordering::SeqCst);
///     signal.set().unwrap();
/// });
///
/// ev.wait_until_set().unwrap();
/// assert_eq!(v.load(Ordering::SeqCst), 1);
///
/// ```
#[derive(Clone)]
pub struct ManualResetEvent(WaitEvent);

/// Wrapper of [`WaitEvent`] of type `bool`, which focuses on waiting for `true` with automatic reset to `false`.
///
/// *Examples*
///
/// ```rust
/// # use sync_wait_object::windows::AutoResetEvent;
/// use std::{ thread, sync::Arc, sync::atomic::{ AtomicU8, Ordering } };
/// use sync_wait_object::SignalWaitable;
///
/// let ev = AutoResetEvent::new();
/// let mut signal = ev.clone();
///
/// let mut next = AutoResetEvent::new();
/// let wait_next = next.clone();
///
/// let v = Arc::new(AtomicU8::new(0));
/// let v_setter = v.clone();
///
/// thread::spawn(move || {
///     v_setter.store(1, Ordering::SeqCst);
///     signal.set().unwrap();
///
///     wait_next.wait_until_set().unwrap();
///
///     v_setter.store(2, Ordering::SeqCst);
///     signal.set().unwrap();
/// });
///
/// ev.wait_until_set().unwrap();
/// assert_eq!(v.load(Ordering::SeqCst), 1);
///
/// next.set().unwrap();
///
/// ev.wait_until_set().unwrap();
/// assert_eq!(v.load(Ordering::SeqCst), 2);
/// ```
#[derive(Clone)]
pub struct AutoResetEvent(WaitEvent);

// ---------------------------------------- FUNCTIONS -------------------------------------------------
#[inline]
pub(crate) fn get_win32_last_error() -> WIN32_ERROR {
    unsafe { GetLastError() }
}

#[inline]
pub(crate) fn get_last_error() -> WaitObjectError {
    get_win32_last_error().into()
}

pub(crate) fn to_result(ret: bool) -> Result<()> {
    if ret { Ok(()) }
    else { Err(get_last_error()) }
}

pub trait HandleWrapper {
    fn handle(&self) -> HANDLE;
}

// ---------------------------------------- IMPLEMENTATIONS -------------------------------------------
impl From<WIN32_ERROR> for WaitObjectError {
    fn from(value: WIN32_ERROR) -> Self {
        WaitObjectError::OsError(value.0 as isize, value.to_hresult().message().to_string())
    }
}

impl WaitEvent {
    fn native_wait(&self, timeout: u32) -> Result<bool> {
        let ret = unsafe { WaitForSingleObject(self.0, timeout) };
        match ret {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            WAIT_FAILED => Err(get_last_error()),
            _ => unreachable!()
        }
    }
}

impl HandleWrapper for WaitEvent {
    #[inline]
    fn handle(&self) -> HANDLE { self.0 }
}

impl SignalWaitable for WaitEvent {
    #[inline]
    fn wait_until_set(&self) -> Result<bool> {
        self.native_wait(INFINITE)
    }

    #[inline] fn wait(&self, timeout: Duration) -> Result<bool> {
        self.native_wait(timeout.as_millis() as u32)
    }

    fn set(&mut self) -> Result<()> {
        to_result(unsafe { SetEvent(self.0).as_bool() })
    }
    fn reset(&mut self) -> Result<()> {
        to_result(unsafe { ResetEvent(self.0).as_bool() })
    }
}

impl Drop for WaitEvent {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe { CloseHandle(self.0); }
            self.0 = HANDLE::default();
        }
    }
}

impl ManualResetEvent {
    #[inline]
    pub fn new() -> Self { Self::new_init(false) }

    pub fn new_init(initial_state: bool) -> Self {
        let handle = unsafe { CreateEventA(None, true, initial_state, None).unwrap() };
        Self(WaitEvent(handle))
    }
}

impl Deref for ManualResetEvent {
    type Target = WaitEvent;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ManualResetEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AutoResetEvent {
    #[inline]
    pub fn new() -> Self { Self::new_init(false) }

    pub fn new_init(initial_state: bool) -> Self {
        let handle = unsafe { CreateEventA(None, false, initial_state, None).unwrap() };
        Self(WaitEvent(handle))
    }
}

impl Deref for AutoResetEvent {
    type Target = WaitEvent;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AutoResetEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod test {
}