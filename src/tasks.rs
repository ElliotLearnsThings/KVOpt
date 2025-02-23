use core::time;
use std::{collections::HashMap, io::{self, Read}, sync::{Arc, Condvar, Mutex, MutexGuard}, thread::{self}};

use chrono::TimeDelta;

use crate::{buffer::BufferAccess, logger::Logger, Cache};

// run_tasks function
pub fn run_tasks(cache: &Arc<Mutex<Cache>>, _logger: Arc<Mutex<Logger>>) -> Result<(), Box<dyn std::error::Error>> {

    let cv = Arc::new((Mutex::new(false), Condvar::new()));
    let cv_for_tasks = Arc::clone(&cv);
    let cache_for_tasks = Arc::clone(cache);
    // Clone cache for tasks_thread
    let tasks_thread = thread::spawn(move || {
        //println!("Tasks thread started");
        loop {
            let cv_inner = Arc::clone(&cv_for_tasks);
            let cache_for_tasks = Arc::clone(&cache_for_tasks);
            let looped_thread = thread::spawn(move || {
                let (ref lock, ref cvar) = *cv_inner;
                let _started = cvar.wait_while(lock.lock().unwrap(), |started| !*started).unwrap();
                let cache = cache_for_tasks.lock().expect("Unable to lock cache");
                let kv = &cache.vals;

                //println!("Ran thread tasks");
                //if *cache.should_exit.lock().unwrap() {
                    //exit(0);
                //}

                let kv_guard = kv.lock().expect("Unable to lock kv");
                invalidate_cache(kv_guard).expect("unable to lock");
                return;
            });
            let _ = looped_thread.join();
            thread::sleep(time::Duration::from_secs_f32(10000.0));
        }
    });

    // Clone cache for buffer_thread
    let cache_for_buffer = Arc::clone(&cache);
    let cv_for_buffer = Arc::clone(&cv);

    let mut duration = TimeDelta::zero();

    let buffer_thread = thread::spawn(move || {

        loop {
            if !duration.is_zero() {
                //println!("diff: {}", duration);
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


fn invalidate_cache(mut kv: MutexGuard<HashMap<[u8; 63], [u8; 64]>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut keys_to_remove: Vec<[u8;63]> = Vec::new();

    for val in kv.iter_mut() {
        let mut should_remove = false;
        let key = val.0;

        // Create start time buffer
        let mut start_time: [u8; 8] = [0u8; 8];
        start_time[2..].copy_from_slice(&val.1[56..62]);

        let start_time = i64::from_be_bytes(start_time);
        //println!("Got start_time: {}", start_time.clone());

        // Create expire time buffer
        let mut expire_time = [0u8;2]; // 56-64 are time values
        expire_time[..].clone_from_slice(&val.1[62..64]);

        let expire_time = i16::from_be_bytes(expire_time);
        //println!("Got expire_time: {}", expire_time.clone());

        // start time is epoch timestamp in secs
        // therefore start_time + hours is expire_time timestamp
        
        let current_timestamp = chrono::Utc::now().timestamp();
        let expire_timestamp = start_time + (expire_time as i64);

        // If the expire is smaller than current
        if current_timestamp > expire_timestamp {
            should_remove = true;
        }

        // If should remove save key to vec
        if should_remove {
            keys_to_remove.push(*key);
        }
    }

    // Remove collected items.
    for item in keys_to_remove {
        kv.remove(&item);
    }

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
            let fbreak = "0".repeat(40);
            let mut tcurb = [0u8;6];
            let tcur = chrono::Utc::now().timestamp().to_be_bytes();
            tcurb[..].copy_from_slice(&tcur[2..]);
            let tlen = (10 as i16).to_be_bytes();
            
            let mut tbuf = [0u8;8];
            tbuf[..6].copy_from_slice(&tcurb);
            tbuf[6..8].copy_from_slice(&tlen);

            let tbufstr = std::str::from_utf8(&tbuf).unwrap();

            let buffer = format!("{}{}{}{}{}{}", command, iuuid, ibreak, fuuid, fbreak, tbufstr);
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

