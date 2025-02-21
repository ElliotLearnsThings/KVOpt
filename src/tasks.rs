use core::time;
use std::{collections::HashMap, io::{self, Read}, sync::{Arc, Condvar, Mutex, MutexGuard}, thread::{self}};

use chrono::TimeDelta;

use crate::{buffer::BufferAccess, logger::Logger, Cache};

// run_tasks function
pub fn run_tasks(cache: Arc<Mutex<Cache>>, _logger: Arc<Mutex<Logger>>) -> Result<(), Box<dyn std::error::Error>> {

    let cv = Arc::new((Mutex::new(false), Condvar::new()));
    let cv_for_tasks = Arc::clone(&cv);
    let cache_for_tasks = Arc::clone(&cache);
    // Clone cache for tasks_thread
    let tasks_thread = thread::spawn(move || {
        println!("Tasks thread started");
        loop {
            let cv_inner = Arc::clone(&cv_for_tasks);
            let cache_for_tasks = Arc::clone(&cache_for_tasks);
            let looped_thread = thread::spawn(move || {
                let (ref lock, ref cvar) = *cv_inner;
                let _started = cvar.wait_while(lock.lock().unwrap(), |started| !*started).unwrap();
                let cache = cache_for_tasks.lock().expect("Unable to lock cache");
                let kv = &cache.vals;

                println!("Ran thread tasks");

                let kv_guard = kv.lock().expect("Unable to lock kv");
                invalidate_cache(kv_guard).expect("unable to lock");
                return;
            });
            let _ = looped_thread.join();
            thread::sleep(time::Duration::from_secs_f32(10.0));
        }
    });

    // Clone cache for buffer_thread
    let cache_for_buffer = Arc::clone(&cache);
    let cv_for_buffer = Arc::clone(&cv);

    let mut duration = TimeDelta::zero();

    let buffer_thread = thread::spawn(move || {

        loop {
            if !duration.is_zero() {
                println!("diff: {}", duration);
            }
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            let mut buffer = [0u8; 128];
            if let Ok(_) = handle.read(&mut buffer) {
                let start_time = chrono::Utc::now();
                let mut cache = cache_for_buffer.lock().expect("Could not lock buffer cache");
                let _ = cache.handle_in(buffer);
                let end_time = chrono::Utc::now();
                duration = end_time - start_time;
                
                let (ref lock, ref cvar) = *cv_for_buffer;
                let mut started = lock.lock().unwrap();
                *started = true;
                cvar.notify_one();
            };
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

#[cfg(test)]

mod tests {
    use super::*;

    #[test] 
    fn max_per_second_insert() {
        let mut cache = Cache::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log");

        let mut buffers: Vec<[u8; 128]> = vec![];

        for _ in 0..1_000_000 {
            let command = "I";
            let iuuid = uuid::Uuid::new_v4().to_string();
            let ibreak = "0".repeat(48);
            let fuuid = uuid::Uuid::new_v4().to_string();
            let fbreak = "0".repeat(48);
            let buffer = format!("{}{}{}{}{}", command, iuuid, ibreak, fuuid, fbreak);
            let buffer = buffer.as_bytes();
            let len = buffer.len().min(128);
            let mut byte_array = [0u8;128];
            byte_array[..len].copy_from_slice(&buffer[..len]);
            buffers.push(byte_array);
        }

        let time_start = chrono::Utc::now();
        for buffer in buffers.iter() {
            let _ = cache.handle_in(*buffer);
        }
        let time_end = chrono::Utc::now();
        let diff = time_end - time_start;
        println!("Delta: {}", diff);
    }
}

