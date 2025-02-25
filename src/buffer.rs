use std::{io::{self, Read, Write}, sync::Arc};

use chrono::Utc;

use crate::Cache;

pub trait BufferAccess<'a> {
    fn _read(&mut self) -> Result<[u8; 128], Box<dyn std::error::Error>>;
    fn handle_in(&'a mut self, input: [u8;128]) -> Result<(), Box<dyn std::error::Error>>;
}

impl<'a> BufferAccess<'a> for Cache {

    fn _read(&mut self) -> Result<[u8; 128], Box<dyn std::error::Error>> {
        let mut input_buf = self.cur_buf.lock().map_err(|_| "Mutex lock failed")?;
        io::stdin().read(&mut *input_buf)?;
        Ok(*input_buf)
    }

    fn handle_in(&mut self, input: [u8; 128]) -> Result<(), Box<dyn std::error::Error>> {
        //let input = self._read().map_err(|_| "Failed reading")?;

        // Clone vals for thread-safe access
        self.log_debug("RUNNING INVALIDATE".to_string());
        let init_time = Utc::now();

        {
            let _ = self.invalidate_cache().expect("unable to invalidate");
            let final_time = Utc::now();
            let time_delta = final_time - init_time;
            self.log_debug(format!("FINISHED INVALIDATE IN {} MILISECONDS", time_delta.num_milliseconds()));
        }
        let vals = Arc::clone(&self.vals);
        //let should_exit = Arc::clone(&self.should_exit);
        let command = input[0];
        let key: [u8; 63] = input[1..64].try_into().expect("Slice length must be 63");
        let value: [u8; 64] = input[64..128].try_into().expect("Slice length must be 64"); 

        match command {
            b'G' => {
                let kv = vals.lock().expect("Unable to lock KV in thread");
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
                let mut kv = vals.lock().expect("Unable to lock KV in thread");
                let _ = kv.remove(&key);
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                handle.write_all(b"R\n").unwrap();
                handle.flush().unwrap();
            }
            
            b'I' => {
                let mut kv = vals.lock().expect("Unable to lock KV in thread");
                self.log_debug(format!("ADDING KV"));
                let _ = kv.insert(key, value);
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                handle.write_all(b"I\n").unwrap();
                handle.flush().unwrap();
            }

            b'H' => {
                match self.clean_up(){
                    Ok(v) => {v},
                    Err(e) => {println!("An error occured in clean_up: {}", e.to_string());}
                };
            }
            _ => {}, // Early return for unrecognized command
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Helper to create a Cache with a pre-filled buffer
    fn setup_cache_with_buffer(buffer: [u8; 128]) -> Cache {
        let cache = Cache::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log", crate::LogLevel::DEBUG);
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

        let cache = Cache::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log", crate::LogLevel::DEBUG);
        cache.vals.lock().unwrap().insert(key, full_value);

        let buf = create_test_buffer(b'G', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        //let result = cache.handle_in();
        //assert!(result.is_ok(), "handle_in failed: {:?}", result.err());

        let kv = cache.vals.lock().unwrap();
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

        let cache = Cache::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log", crate::LogLevel::DEBUG);
        cache.vals.lock().unwrap().insert(key, full_value);

        let buf = create_test_buffer(b'R', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        //let result = cache.handle_in();
        //assert!(result.is_ok(), "handle_in failed: {:?}", result.err());

        let kv = cache.vals.lock().unwrap();
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

        //let result = cache.handle_in();
        //assert!(result.is_ok(), "handle_in failed: {:?}", result.err());

        let kv = cache.vals.lock().unwrap();
        assert_eq!(kv.get(&key), Some(&full_value));
    }

    #[test]
    fn test_handle_in_halt() {
        let key = [1; 63];
        let value = [2; 60];
        let expiration = b"0010";

        let buf = create_test_buffer(b'H', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        //let result = cache.handle_in();
        //assert!(result.is_ok(), "handle_in failed: {:?}", result.err());

        let should_exit = cache.should_exit.lock().unwrap();
        assert_eq!(*should_exit, true);
    }

    #[test]
    fn test_handle_in_unknown_command() {
        let key = [1; 63];
        let value = [2; 60];
        let expiration = b"0010";

        let buf = create_test_buffer(b'X', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        //let result = cache.handle_in();
        //assert!(result.is_ok(), "handle_in failed: {:?}", result.err());

        let kv = cache.vals.lock().unwrap();
        assert!(kv.is_empty());
        let should_exit = cache.should_exit.lock().unwrap();
        assert_eq!(*should_exit, false);
    }

    #[test]
    fn test_handle_in_with_expiration() {
        let key = [1; 63];
        let value = [2; 60];
        let expiration = b"0010"; // 2 hours in ASCII
        let mut full_value = [0; 64];
        full_value[0..60].copy_from_slice(&value);
        full_value[60..64].copy_from_slice(expiration);

        let buf = create_test_buffer(b'I', &key, &value, expiration);
        let cache = setup_cache_with_buffer(buf);

        //let result = cache.handle_in();
        //assert!(result.is_ok(), "handle_in failed: {:?}", result.err());

        let kv = cache.vals.lock().unwrap();
        assert_eq!(kv.get(&key), Some(&full_value));
        // Expiration isn’t parsed yet; this verifies storage
    }
}

