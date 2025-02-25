use std::{io::{self, Read, Write}, sync::Arc};
use chrono::Utc;
use crate::Cache;

pub trait BufferAccess<'a> {
    fn _read(&mut self) -> Result<[u8; 128], Box<dyn std::error::Error>>;
    fn handle_in(&'a mut self, input: [u8;128]) -> Result<(), Box<dyn std::error::Error>>;
    fn handle_batch(&'a mut self, inputs: &[[u8;128]]) -> Result<Vec<Result<(), Box<dyn std::error::Error>>>, Box<dyn std::error::Error>>;
}

impl<'a> BufferAccess<'a> for Cache {
    fn _read(&mut self) -> Result<[u8; 128], Box<dyn std::error::Error>> {
        let mut input_buf = self.cur_buf.lock().map_err(|_| "Mutex lock failed")?;
        io::stdin().read(&mut *input_buf)?;
        Ok(*input_buf)
    }

    fn handle_in(&mut self, input: [u8; 128]) -> Result<(), Box<dyn std::error::Error>> {
        // Optimize by invalidating cache only periodically, not on every operation
        if self.ops_since_invalidation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= self.invalidation_threshold {
            self.invalidate_cache()?;
            self.ops_since_invalidation.store(0, std::sync::atomic::Ordering::SeqCst);
        }

        // Get command and references to key/value bytes without unnecessary copying
        let command = input[0];
        
        // We do need to copy for HashMap keys, but we'll minimize unnecessary copies
        let key_slice = &input[1..64];
        
        // Check if key is empty or all zeros (treat as invalid)
        let is_empty_key = key_slice.iter().all(|&b| b == 0);
        if is_empty_key && (command == b'I' || command == b'G') {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(b"E\n").unwrap(); // Return error code for empty key
            handle.flush().unwrap();
            return Ok(());
        }
        
        let value_slice = &input[64..128];
        
        let vals = Arc::clone(&self.vals);

        match command {
            b'G' => {
                // Optimize by using RwLock's read access for get operations
                let kv = vals.read().expect("Unable to get read lock on KV in thread");
                let mut key = [0u8; 63];
                key.copy_from_slice(key_slice);
                
                if let Some(out) = kv.get(&key) {
                    let stdout = io::stdout();
                    let mut handle = stdout.lock();
                    handle.write_all(out).unwrap();
                    handle.write(b"\n").unwrap();
                    handle.flush().unwrap();
                } else {
                    let stdout = io::stdout();
                    let mut handle = stdout.lock();
                    handle.write_all(b"G").unwrap();
                    handle.write(b"\n").unwrap();
                    handle.flush().unwrap();
                };
            },

            b'R' => {
                let mut kv = vals.write().expect("Unable to lock KV in thread");
                let mut key = [0u8; 63];
                key.copy_from_slice(key_slice);
                
                // Also remove from entries map
                self.entries.remove(&key);
                
                let _ = kv.remove(&key);
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                handle.write_all(b"R\n").unwrap();
                handle.flush().unwrap();
                
                // Force save on remove to ensure persistence
                self.save_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            
            b'I' => {
                let mut kv = vals.write().expect("Unable to lock KV in thread");
                if self.level == crate::LogLevel::DEBUG {
                    self.log_debug(format!("ADDING KV"));
                }
                
                let mut key = [0u8; 63];
                let mut value = [0u8; 64];
                key.copy_from_slice(key_slice);
                value.copy_from_slice(value_slice);
                
                // Extract expiration time and timestamp for the cache entry
                let expire_time_seconds = u16::from_be_bytes([value[62], value[63]]);
                
                // Set timestamp to now
                let timestamp = Utc::now();
                
                // Calculate expiration time
                let expires_at = if expire_time_seconds > 0 {
                    Some(timestamp + chrono::TimeDelta::try_seconds(expire_time_seconds as i64).unwrap_or_default())
                } else {
                    None
                };
                
                // Create and store cache entry
                let mut value_only = [0u8; 56];
                value_only.copy_from_slice(&value[0..56]);
                
                let entry = crate::CacheEntry {
                    value: value_only,
                    created_at: timestamp,
                    expires_at,
                };
                
                // Store in both collections
                self.entries.insert(key, entry);
                let _ = kv.insert(key, value);
                
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                handle.write_all(b"I\n").unwrap();
                handle.flush().unwrap();
                
                // Flag for save on insert to ensure persistence
                self.save_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }

            b'H' => {
                if let Err(e) = self.clean_up() {
                    if self.level == crate::LogLevel::DEBUG {
                        println!("An error occurred in clean_up: {}", e);
                    }
                }
            }
            _ => {}, // Early return for unrecognized command
        };

        Ok(())
    }

    // New batch processing method for improved throughput
    fn handle_batch(&'a mut self, inputs: &[[u8;128]]) -> Result<Vec<Result<(), Box<dyn std::error::Error>>>, Box<dyn std::error::Error>> {
        let mut results = Vec::with_capacity(inputs.len());
        
        // Process all commands in batch
        for input in inputs {
            results.push(self.handle_in(*input));
        }
        
        // Always force a save after batch operations
        self.save_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Helper to create a Cache with a pre-filled buffer
    fn setup_cache_with_buffer(buffer: [u8; 128]) -> Cache {
        let cache = Cache::new("/tmp/cache_test.log", crate::LogLevel::DEBUG);
        let buf = Arc::clone(&cache.cur_buf);
        let mut buf = buf.lock().unwrap();
        *buf = buffer;
        cache
    }

    // Helper to create a test buffer (command + 63-byte key + 60-byte value + 4-byte expiration)
    fn create_test_buffer(command: u8, key: &[u8; 63], value: &[u8; 60], expiration: &[u8; 4]) -> [u8; 128] {
        let mut buf = [0; 128];
        buf[0] = command;
        buf[1..64].copy_from_slice(key);
        buf[64..124].copy_from_slice(value);
        buf[124..128].copy_from_slice(expiration);
        buf
    }

    #[test]
    fn test_handle_in_get() {
        let key = [1; 63];
        let value = [2; 60];
        let expiration = b"0010"; // 2 hours
        let mut full_value = [0; 64];
        full_value[0..60].copy_from_slice(&value);
        full_value[60..64].copy_from_slice(expiration);

        let cache = Cache::new("/tmp/cache_test.log", crate::LogLevel::DEBUG);
        cache.vals.write().unwrap().insert(key, full_value);

        let buf = create_test_buffer(b'G', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        let kv = cache.vals.read().unwrap();
        assert_eq!(kv.get(&key), Some(&full_value));
    }

    #[test]
    fn test_handle_in_remove() {
        let key = [1; 63];
        let value = [2; 60];
        let expiration = b"0010";
        let mut full_value = [0; 64];
        full_value[0..60].copy_from_slice(&value);
        full_value[60..64].copy_from_slice(expiration);

        let cache = Cache::new("/tmp/cache_test.log", crate::LogLevel::DEBUG);
        cache.vals.write().unwrap().insert(key, full_value);

        let buf = create_test_buffer(b'R', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        let kv = cache.vals.read().unwrap();
        assert_eq!(kv.get(&key), None);
    }

    #[test]
    fn test_handle_in_insert() {
        let key = [1; 63];
        let value = [2; 60];
        let expiration = b"0010";
        let mut full_value = [0; 64];
        full_value[0..60].copy_from_slice(&value);
        full_value[60..64].copy_from_slice(expiration);

        let buf = create_test_buffer(b'I', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        let kv = cache.vals.read().unwrap();
        assert_eq!(kv.get(&key), Some(&full_value));
    }

    #[test]
    fn test_handle_batch() {
        let mut buffers = Vec::new();
        
        // Create 3 insert operations
        for i in 0..3 {
            let mut key = [0u8; 63];
            let mut value = [0u8; 60];
            let expiration = b"0010";
            
            // Create unique key and value
            key[0] = i + 1;
            value[0] = i + 10;
            
            buffers.push(create_test_buffer(b'I', &key, &value, expiration));
        }
        
        let mut cache = Cache::new("/tmp/cache_test.log", crate::LogLevel::DEBUG);
        let result = cache.handle_batch(&buffers);
        
        assert!(result.is_ok());
        
        // Verify all 3 keys were inserted
        let kv = cache.vals.read().unwrap();
        for i in 0..3 {
            let mut key = [0u8; 63];
            key[0] = i + 1;
            assert!(kv.contains_key(&key));
        }
    }
    
    #[test]
    fn test_empty_key() {
        let key = [0u8; 63]; // Empty key
        let value = [2; 60];
        let expiration = b"0010";
        
        let buf = create_test_buffer(b'I', &key, &value, expiration);
        let mut cache = setup_cache_with_buffer(buf);
        let result = cache.handle_in(buf);
        
        assert!(result.is_ok());
        
        // Empty key should not be inserted
        let kv = cache.vals.read().unwrap();
        assert!(!kv.contains_key(&key));
    }
}

