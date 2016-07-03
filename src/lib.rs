pub use self::monitor::Monitor as Monitor;
pub use self::monitor::MonitorGuard as MonitorGuard;

mod monitor;

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use {Monitor, MonitorGuard};
     
    use std::time::{Duration, Instant};
 
    fn template_one_waiter<FDS, FDW, FTW> (sleep_time: Duration, wait_time: Duration,
                                           f_done_sleeping: FDS,
                                           f_done_waiting: FDW,
                                           f_timeout_waiting: FTW)
     where FDS : Fn(MonitorGuard<bool>) + Send + 'static,
           FDW : Fn(),
           FTW : Fn()
     {
        let mon = Arc::new(Monitor::new(false));
        {
            let mon = mon.clone();
            let _ = thread::spawn(move || {
                thread::sleep(sleep_time);
                mon.with_lock(|done: MonitorGuard<bool>| {
                    f_done_sleeping(done);
                });
            });
        }
        
        mon.with_lock(|mut done| {
            let start = Instant::now();
            let mut now = start;
//            let end_time = curr_time + wait_time;
            
//            while !*done && curr_time < end_time {
            while !*done && now-start < wait_time {
                done.wait_timeout(start+wait_time-now);
                now = Instant::now();
            }
            
            if *done {
                f_done_waiting();
            } else {
                f_timeout_waiting();
            }
        });
     }
     
     
     
    fn template_n_waiters<FDS, FDW, FTW> (sleep_time: Duration, wait_times: &[Duration],
                                          f_done_sleeping: FDS,
                                          f_done_waiting: FDW,
                                          f_timeout_waiting: FTW)
    where FDS : Fn(MonitorGuard<u32>) + Send + 'static,
          FDW : Fn(u32) + Send + Sync + 'static,
          FTW : Fn(u32) + Send + Sync + 'static
    {
        let mon = Arc::new(Monitor::new(0));
        {
            let mon = mon.clone();
            let _ = thread::spawn(move || {
                thread::sleep(sleep_time);
                mon.with_lock(|done: MonitorGuard<u32>| {
                    f_done_sleeping(done);
                });
            });
        }
        
        
        struct Closure<FDW: Fn(u32) + Send + Sync + 'static, FTW: Fn(u32) + Send + Sync + 'static> {
            f_done_waiting: FDW,
            f_timeout_waiting: FTW
        };
        let closure = Arc::new(Closure {f_done_waiting:f_done_waiting, f_timeout_waiting:f_timeout_waiting});
        
        
        let waiter = |thread_id, wait_time, closure: Arc<Closure<FDW, FTW>>| {
            move |mut done: MonitorGuard<u32>| {
                let start = Instant::now();
                let mut now = start;
                while *done == 0 && now-start < wait_time {
                    done.wait_timeout(start+wait_time-now);
                    now = Instant::now();
                }
                
                if *done > 0 {
                    (closure.f_done_waiting)(thread_id);
                    *done = *done - 1;
                } else {
                    (closure.f_timeout_waiting)(thread_id);
                }
            }
        };
        
        let threads : Vec<_> = wait_times.into_iter().enumerate().map(|(i,t)| {
            let mon = mon.clone();
            let waiter = waiter(i as u32, t.clone(), closure.clone());
            thread::Builder::new().name(format!("{:?}", t)).spawn(move || {
                mon.with_lock(waiter);
            }).unwrap()
        }).collect();
        
        
        for t in threads {
            if let Err(x) = t.join() {
                let z : &&'static str = x.downcast_ref().unwrap();
                panic!(*z);
            } 
        }
     }
 
    #[test]
    fn test_waking() {
        let mon = Arc::new(Monitor::new(false));
        {
            let mon = mon.clone();
            let _ = thread::spawn(move || {
                thread::sleep(Duration::from_millis(500));
                mon.with_lock(|mut done| {
                    *done = true;
                    done.notify_one();
                });
            });
        }
        
        mon.with_lock(|mut done| {
            let timeout = Duration::from_millis(1000);
            let start = Instant::now();
            let mut now = start;
            
            while !*done {
                if now >= start+timeout {
                    panic!("timeout reached");
                }
                done.wait_timeout(start+timeout-now);
                now = Instant::now();
            }
        });
    }
    
    
    
    fn d50() -> Duration { Duration::from_millis(50) }
    fn d100() -> Duration { Duration::from_millis(100) }
    
    
    #[test]
    fn test_notify_one_should_timeout() {
        template_one_waiter(d100().clone(), d50().clone(),
                 |mut done| {
                    *done = true;
                    done.notify_one();
                 },
                 
                 || panic!("should have timed out"),
                 || {}
        );
        
        template_one_waiter(d50().clone(), d100().clone(),
                 |done| {
                    done.notify_one();
                 },
                 
                 || panic!("should have timed out"),
                 || {}
        );
    }
        
    #[test]
    fn test_notify_one_should_not_timeout() {
        template_one_waiter(d50().clone(), d100().clone(),
                 |mut done| {
                    *done = true;
                    done.notify_one();
                 },
                 
                 || {},
                 || panic!("should not time out")
        );
        
        template_one_waiter(d50().clone(), d100().clone(),
                 |mut done| {
                    *done = true;
                    done.notify_one();
                    thread::sleep(Duration::from_millis(100))
                 },
                 
                 || {},
                 || panic!("should not time out")
        );
        
        template_one_waiter(d50().clone(), d100().clone(),
                 |mut done| {
                    done.notify_one();
                    thread::sleep(Duration::from_millis(100));
                    *done = true;
                 },
                 
                 || {},
                 || panic!("should not time out")
        );
    }
    
    
    #[test]
    fn test_notify_all_should_timeout() {
        template_n_waiters(d100().clone(), &[d50().clone()],
                 |mut done| {
                    *done = 1;
                    done.notify_one();
                 },
                 
                 |tid| panic!("thread {} should have timed out", tid),
                 |_| {}
        );
        
        template_n_waiters(d100().clone(), &[d50().clone()],
                 |mut done| {
                    *done = 1;
                    done.notify_one();
                 },
                 
                 |tid| panic!("thread {} should have timed out", tid),
                 |_| {}
        );
    }
        
    #[test]
    fn test_notify_one_where_1_should_timeout() {
        let c = Arc::new(Mutex::new(0));
        let _c = c.clone();
        template_n_waiters(d50().clone(), &[d100().clone(), d100().clone()],
                 |mut done| {
                    *done = 1;
                    done.notify_one();
                 },
                 
                 
                 move |_| {
                     let mut c = _c.lock().unwrap();
                     *c = *c+1
                 },
                 
                 |_| {}
        );
        
        let c = c.lock().unwrap();
        if 2-*c != 1 {
            panic!("exactly one thread should have timed out, but {} did", 2-*c);
        }
    }
    
        
    #[test]
    fn test_notify_all_should_not_timeout_1() {
        template_n_waiters(d50().clone(), &[d100().clone()],
                 |mut done| {
                    *done = 1;
                    done.notify_all();
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone()],
                 |mut done| {
                    *done = 1;
                    done.notify_all();
                    thread::sleep(Duration::from_millis(100))
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(Duration::from_millis(100));
                    *done = 1;
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(Duration::from_millis(100));
                    *done = 1;
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
    }
    
        
    #[test]
    fn test_notify_all_should_not_timeout_2() {
        template_n_waiters(d50().clone(), &[d100().clone(), d100().clone()],
                 |mut done| {
                    *done = 2;
                    done.notify_all();
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone(), d100().clone()],
                 |mut done| {
                    *done = 2;
                    done.notify_all();
                    thread::sleep(Duration::from_millis(100))
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone(), d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(Duration::from_millis(100));
                    *done = 2;
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone(), d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(Duration::from_millis(100));
                    *done = 2;
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
    }
}
