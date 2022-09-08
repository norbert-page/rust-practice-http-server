#![forbid(unsafe_code)]
// As this is an exercise project, we generate documentation for internal dev use only
// and use `cargo doc --open --document-private-items`
#![allow(rustdoc::private_intra_doc_links)]

//! Thread-pool; simple implementation with graceful shutdown for
//! use in a basic multi-threaded [*HTTP server*](../server/index.html) written for practice.
//!
//! Uses [Job] type for jobs (closures) to be executed. Implementation uses 
//! [std::sync::mpsc] to send jobs across threads. There's also a simple [unit test](tests)
//! implemented with [std::sync::atomic::AtomicUsize].
//!
//! # Example
//! (and basic doctest)
//!
//! ```
//! # use threadpool::ThreadPool;
//! #
//! # fn main() {
//!     let pool = ThreadPool::new(4);
//!
//!     for n in 0..16 {
//!         pool.execute(move || {
//!             println!("{n}: job done.");
//!         });
//!     }
//! # }
//! ```
//!
//! # Safety
//! Uses `#![forbid(unsafe_code)]` attribute.
//! 
//! # Panics
//! [`new`](ThreadPool::new) panics when `size` parameter is 0 (it's not ideal for
//!  it to panic, but it's just a practice project). Function [`build`](ThreadPool::build)
//!  does not panic on `size` equal to 0 and returns error instead.

use std::{
    error,
    fmt::{self, Display, Formatter},
    sync::{mpsc, Arc, Mutex},
    thread,
};

/// Thread pool with a given number of threads.
// [Job]s on threads are managed, received and executed by [Worker]s.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

/// Alias for a type of closures to be executed on a thread-pool.
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// Returned by [build](ThreadPool::build) function for `size` argument of 0.
#[derive(Debug)]
pub struct InvalidPoolSizeError;

impl Display for InvalidPoolSizeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &self)
    }
}

impl error::Error for InvalidPoolSizeError {}

impl ThreadPool {
    /// Create a new ThreadPool
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    /// Create a new ThreadPool
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `build` function, in contrast to `new`, will not panic if the size is incorrect.
    pub fn build(size: usize) -> Result<ThreadPool, InvalidPoolSizeError> {
        if size == 0 {
            Err(InvalidPoolSizeError)
        } else {
            Ok(ThreadPool::new(size))
        }
    }

    // Execute a closure on the thread pool.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    /// Graceful shutdown for a thread pool, waits for all Workers to finish current jobs, if any.
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

/// Receives and manages execution of [Jobs](Job) (closures) on a single thread.
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    /// Create a worker to continuously receive closures (work) via a thread-safe
    /// multiple producers, single consumer channel.
    ///
    /// Receiver uses a `Mutex` to enable multiple consumers.
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            match receiver.lock().unwrap().recv() {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");

                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

/// Tests. 
/// 
/// Use `source` link on the right side of the page to see them.
// Commented out to mention tests module with cargo doc.
// #[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use crate::ThreadPool;

    #[test]
    fn basic_counter() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        const N_JOBS: usize = 16;

        // atomics for a thread-safe mutability
        let counter = Arc::new(AtomicUsize::new(0));

        // new scope: we want ThreadPool to be dropped before assert check,
        // so that it waits for all jobs to finish
        // alternatively, we could use std::mem::drop() before assert
        {
            let pool = ThreadPool::new(4);

            for _ in 0..N_JOBS {
                let counter = Arc::clone(&counter);
                pool.execute(move || {
                    // add one to the shared counter
                    counter.fetch_add(1, Ordering::Relaxed);
                });
            }
        }

        assert_eq!(N_JOBS, counter.load(Ordering::Relaxed));
    }
}
