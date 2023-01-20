# Wait Object based on Mutex and Condvar

Provide an abstraction over `Condvar` + `Mutex` usage, as provided by the Rust document 
in [Condvar](https://doc.rust-lang.org/std/sync/struct.Condvar.html).

The library provides three main types: `WaitEvent`, `ManualResetEvent`, and `AutoResetEvent`. `WaitEvent` is the core
abstraction mentioned. `ManualResetEvent` and `AutoResetEvent` are just a specialization for `bool` type.

When compiling with Windows platform, the lib also provides `windows` module for native implementation of
`ManualResetEvent` and `AutoResetEvent`.

Example of the abstraction provided:

```rust
use sync_wait_object::WaitEvent;
use std::thread;

let wait3 = WaitEvent::new_init(0);
let mut wait_handle = wait3.clone();

thread::spawn(move || {
    for i in 1..=3 {
        wait_handle.set_state(i).unwrap();
    }
});

let timeout = std::time::Duration::from_secs(1);
let r#final = *wait3.wait(Some(timeout), |i| *i == 3).unwrap();
let current = *wait3.value().unwrap();
assert_eq!(r#final, 3);
assert_eq!(current, 3);
```

The second is to wait and then reset the value to a desired state.
```rust
use sync_wait_object::WaitEvent;
use std::thread;

let wait3 = WaitEvent::new_init(0);
let mut wait_handle = wait3.clone();

thread::spawn(move || {
    for i in 1..=3 {
        wait_handle.set_state(i).unwrap();
    }
});

let timeout = std::time::Duration::from_secs(1);
let r#final = wait3.wait_reset(Some(timeout), || 1, |i| *i == 3).unwrap();
let current = *wait3.value().unwrap();
assert_eq!(r#final, 3);
assert_eq!(current, 1);
```
