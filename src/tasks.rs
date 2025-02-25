use std::{io::{self, Read}, sync::{Arc, Mutex}, time::Duration};
use chrono::Utc;
use crate::{buffer::BufferAccess, Cache};

// Optimized input buffer size for better throughput
const INPUT_BUFFER_SIZE: usize = 128 * 16; // 16 commands at once
// How often to persist cache to disk (in seconds)
const PERSISTENCE_INTERVAL_SECS: u64 = 60;

// run_tasks function, optimized for throughput and efficiency
pub fn run_tasks(cache: &Arc<Mutex<Cache>>) -> Result<(), Box<dyn std::error::Error>> {
    // Log initial state
    {
        let mut cache_lock = cache.lock().unwrap();
        let kv_size = cache_lock.vals.read().unwrap().len();
        cache_lock.log_debug(format!("Starting cache service with {} entries", kv_size));
    }
    
    // Create a background task for periodic persistence
    let persistence_cache = Arc::clone(cache);
    std::thread::spawn(move || {
        loop {
            // Sleep for the persistence interval
            std::thread::sleep(Duration::from_secs(PERSISTENCE_INTERVAL_SECS));
            
            // Check if we should exit
            {
                let cache_lock = persistence_cache.lock().unwrap();
                if cache_lock.should_exit.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
            }
            
            // Persist cache to disk
            if let Ok(mut cache_lock) = persistence_cache.lock() {
                let start_time = Utc::now();
                cache_lock.log_debug("PERIODIC CACHE PERSISTENCE".to_string());
                
                if let Err(e) = cache_lock.clean_up() {
                    eprintln!("Error during periodic persistence: {}", e);
                }
                
                let end_time = Utc::now();
                let duration = end_time - start_time;
                cache_lock.log_debug(format!("Persistence completed in {} ms", duration.num_milliseconds()));
            }
        }
    });
    
    // Create a background task for periodic cache invalidation
    let invalidation_cache = Arc::clone(cache);
    std::thread::spawn(move || {
        loop {
            // Run cache invalidation every 5 seconds
            std::thread::sleep(Duration::from_secs(5));
            
            // Check if we should exit
            {
                let cache_lock = invalidation_cache.lock().unwrap();
                if cache_lock.should_exit.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
            }
            
            // Invalidate expired cache entries
            if let Ok(mut cache_lock) = invalidation_cache.lock() {
                let _ = cache_lock.invalidate_cache();
            }
        }
    });
    
    // Main processing loop - optimized for throughput
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    
    // Pre-allocate buffer for batch processing
    let mut buffer = vec![0u8; INPUT_BUFFER_SIZE];
    let mut command_buffers = Vec::with_capacity(16);
    
    loop {
        // Check if we should exit
        {
            let cache_lock = cache.lock().unwrap();
            if cache_lock.should_exit.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
        }
        
        // Read a batch of commands
        match handle.read(&mut buffer) {
            Ok(bytes_read) if bytes_read > 0 => {
                // Process in chunks of 128 bytes (command size)
                command_buffers.clear();
                
                for chunk in buffer[..bytes_read].chunks_exact(128) {
                    let mut cmd_buffer = [0u8; 128];
                    cmd_buffer.copy_from_slice(chunk);
                    command_buffers.push(cmd_buffer);
                }
                
                if !command_buffers.is_empty() {
                    // Process commands in batch when possible
                    if command_buffers.len() > 1 {
                        if let Ok(mut cache_lock) = cache.lock() {
                            let _ = cache_lock.handle_batch(&command_buffers);
                        }
                    } else {
                        // Process single command
                        if let Ok(mut cache_lock) = cache.lock() {
                            let _ = cache_lock.handle_in(command_buffers[0]);
                        }
                    }
                }
            },
            Ok(_) => {
                // Zero bytes read, pause briefly to avoid CPU spinning
                std::thread::sleep(Duration::from_millis(10));
            },
            Err(e) => {
                eprintln!("Error reading from stdin: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

