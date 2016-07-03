#![allow(dead_code)]

use std::sync::{Mutex, MutexGuard, Condvar, WaitTimeoutResult};
use std::time::Duration;
use std::ops::{Deref, DerefMut};


pub struct Monitor<T: Sized> {
    mutex: Mutex<T>,
    cvar: Condvar,
}



impl<T: Sized> Monitor<T> {
    pub fn new(val: T) -> Monitor<T> {
        Monitor { mutex: Mutex::new(val), cvar: Condvar::new() }
    }
    
    pub fn with_lock<U, F> (&self, f: F) -> U
    where F: FnOnce(MonitorGuard<T>) -> U 
    {
        let g = self.mutex.lock().unwrap();
        f(MonitorGuard::new(&self.cvar, g))
    }
}


pub struct MonitorGuard<'a, T: 'a> {
    cvar: &'a Condvar,
    guard: Option<MutexGuard<'a, T>>
}

impl<'a, T: 'a> MonitorGuard<'a, T> {
    pub fn new(cvar: &'a Condvar, guard: MutexGuard<'a, T>) -> MonitorGuard<'a, T> {
        MonitorGuard { cvar: cvar, guard: Some(guard) }
    }
    
    
    pub fn wait(&mut self) {
        let g = self.cvar.wait(self.guard.take().unwrap()).unwrap();
        self.guard = Some(g)
    }
    
    pub fn wait_timeout(&mut self, t: Duration) -> WaitTimeoutResult {
		let (g, finished) = self.cvar.wait_timeout(self.guard.take().unwrap(), t).unwrap();
        self.guard = Some(g);    	
        finished
    }
    
    pub fn notify_one(&self) {
        self.cvar.notify_one();
    }
    
    pub fn notify_all(&self) {
        self.cvar.notify_all();
    }
}


impl<'a, T> Deref for MonitorGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.guard.as_ref().unwrap()
    }
}

impl<'a, T> DerefMut for MonitorGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.as_mut().unwrap()
    }
}
