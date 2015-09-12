## monitor_rs
A Rust library that implements the [Monitor synchronization construct](https://en.wikipedia.org/wiki/Monitor_%28synchronization%29).

License: [MIT](https://github.com/kirillkh/monitor_rs/blob/master/legal/mit.md)

### Usage
Put this in your `Cargo.toml`:

```toml
[dependencies]
monitor = "*"
```

And this in your crate root:
```rust
extern crate monitor;
use monitor::{Monitor, MonitorGuard};
```


### Example
```rust
extern crate monitor;

use monitor::{Monitor, MonitorGuard};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let mon = Arc::new(Monitor::new(false));
    {
        let mon = mon.clone();
        let _ = thread::spawn(move || {
            thread::sleep(Duration::new(1, 0));
            mon.with_lock(&|done: MonitorGuard<bool>| {
                *done = true;
                done.notify_one();
            });
        });
    }
    
    mon.with_lock(&|mut done| {
        while !*done {
            done.wait();
        }
    });
}
```

For more examples, see the tests in lib.rs.
