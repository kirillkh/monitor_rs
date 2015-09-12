#![feature(wait_timeout)]
#![cfg_attr(test, feature(thread_sleep))]

mod monitor;

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use monitor::Monitor;
    use monitor::MonitorGuard;
     
    extern crate time;
    use self::time::Duration;
    use std::time::Duration as StdDur;
 
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
                thread::sleep(duration_conv(sleep_time));
                mon.with_lock(&|done: MonitorGuard<bool>| {
                    f_done_sleeping(done);
                });
            });
        }
        
        mon.with_lock(&|mut done| {
            let mut curr_time = time::get_time();
            let end_time = curr_time + wait_time;
            
            while !*done && curr_time < end_time {
                done.wait_timeout(duration_conv(end_time - curr_time));
                curr_time = time::get_time();
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
                thread::sleep(duration_conv(sleep_time));
                mon.with_lock(&|done: MonitorGuard<u32>| {
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
            Box::new(move |mut done: MonitorGuard<u32>| {
                let mut curr_time = time::get_time();
                let end_time = curr_time + wait_time;
                
                while *done == 0 && curr_time < end_time {
                    done.wait_timeout(duration_conv(end_time - curr_time));
                    curr_time = time::get_time();
                }
                
                if *done > 0 {
                    (closure.f_done_waiting)(thread_id);
                    *done = *done - 1;
                } else {
                    (closure.f_timeout_waiting)(thread_id);
                }
            })
        };
        
        let threads : Vec<_> = wait_times.into_iter().enumerate().map(|(i,t)| {
            let mon = mon.clone();
            let waiter = waiter(i as u32, t.clone(), closure.clone());
            thread::Builder::new().name(format!("{}", t)).spawn(move || {
                mon.with_lock(&*waiter);
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
                thread::sleep_ms(500);
                mon.with_lock(&|mut done| {
                    *done = true;
                    done.notify_one();
                });
            });
        }
        
        mon.with_lock(&|mut done| {
            let timeout = 1000;
            let mut curr_time = time::get_time();
            let end_time = curr_time + Duration::milliseconds(timeout);
            
            while !*done {
                if curr_time >= end_time {
                    panic!("timeout reached");
                }
                done.wait_timeout(duration_conv(end_time - curr_time));
                curr_time = time::get_time();
            }
        });
    }
    
    
    
    fn d50() -> Duration { Duration::milliseconds(50) }
    fn d100() -> Duration { Duration::milliseconds(100) }
    
    
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
                    thread::sleep(StdDur::from_millis(100))
                 },
                 
                 || {},
                 || panic!("should not time out")
        );
        
        template_one_waiter(d50().clone(), d100().clone(),
                 |mut done| {
                    done.notify_one();
                    thread::sleep(StdDur::from_millis(100));
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
                    thread::sleep(StdDur::from_millis(100))
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(StdDur::from_millis(100));
                    *done = 1;
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(StdDur::from_millis(100));
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
                    thread::sleep(StdDur::from_millis(100))
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone(), d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(StdDur::from_millis(100));
                    *done = 2;
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
        
        template_n_waiters(d50().clone(), &[d100().clone(), d100().clone()],
                 |mut done| {
                    done.notify_all();
                    thread::sleep(StdDur::from_millis(100));
                    *done = 2;
                 },
                 
                 |_| {},
                 |tid| panic!("thread {} should not time out", tid)
        );
    }
    
    
    fn duration_conv(d: Duration) -> StdDur {
        let seconds = d.num_seconds();
        StdDur::new(seconds as u64, (d - Duration::seconds(seconds)).num_nanoseconds().unwrap()  as u32)
    }
}
