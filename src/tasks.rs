use core::time;
use std::{collections::HashMap, sync::{Arc, Mutex, MutexGuard}, thread::{self}};

use crate::{buffer::BufferAccess, logger::{Log, Logger}, Cache};

// run_tasks function
pub fn run_tasks(cache: Arc<Mutex<Cache>>, logger: Arc<Mutex<Logger>>) -> Result<(), Box<dyn std::error::Error>> {

    let tasks_thread_logger = Arc::clone(&logger);
    let buffer_thread_logger = Arc::clone(&logger);

    let cache_for_tasks = Arc::clone(&cache);
    // Clone cache for tasks_thread
    let tasks_thread = thread::spawn(move || {
        let mut logger = tasks_thread_logger.lock().expect("Unable to lock logger");
        logger.write_log(format!("START THREAD TASKS THREAD {}", chrono::Utc::now())).unwrap();

        loop {
            let cache = cache_for_tasks.lock().expect("Unable to lock cache");
            let should_exit = &cache.should_exit;
            let kv = &cache.vals;
            let should_exit_guard = should_exit.lock().expect("Unable to lock should_exit: {}");
            let should_exit = *should_exit_guard;

            if should_exit {
                break
            }
            thread::sleep(time::Duration::from_secs_f64(10.0));
            let kv_guard = kv.lock().expect("Unable to lock kv");
            invalidate_cache(kv_guard).expect("unable to lock");
        }
    });

    // Clone cache for buffer_thread
    let cache_for_buffer = Arc::clone(&cache);
    let buffer_thread = thread::spawn(move || {
        let mut logger = buffer_thread_logger.lock().expect("Unable to lock logger");
        logger.write_log(format!("START BUFFER THREAD {}", chrono::Utc::now())).unwrap();
        loop {
            let mut cache_guard = cache_for_buffer.lock().expect("Unable to lock cache");
            let _ = cache_guard.handle_in();
        }
    });

    // Join threads and propagate errors
    tasks_thread.join().map_err(|_| "Tasks thread panicked")?;
    buffer_thread.join().map_err(|_| "Buffer thread panicked")?;

    Ok(())
}


fn invalidate_cache(_kv: MutexGuard<HashMap<[u8; 63], [u8; 64]>>) -> Result<(), Box<dyn std::error::Error>> {
    // not implemented
    Ok(())
    
}


