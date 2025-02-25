use std::{collections::HashMap, fmt::{self, Display}, io::{Read, Write}, path::Path, sync::{Arc, Mutex, MutexGuard}};

use chrono::Utc;

use crate::{logger::Log, Cache, LogLevel};

impl<T> Log<T> for Cache
where
    T: Display + fmt::Write + std::marker::Send + std::marker::Sync,
{
    // This will be on spawned thread
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>> 
        where T: std::fmt::Display + Send,
    {

        let input_clone = input.to_string();
        let cur_time = Utc::now();

        let path_name = Arc::clone(&self.log_path);

        let log_thread = std::thread::spawn({
            move || {
                let path = path_name.lock().unwrap();
                let mut buf = std::fs::read(&*path).unwrap();
                let log_entry = format!("\n\r[LOG @{}]{}", cur_time, input_clone);
                buf.extend_from_slice(log_entry.as_bytes());
                std::fs::write(&*path, &buf).unwrap();
            }
        });

        log_thread.join().unwrap();
        Ok(())
    }
}

impl Cache {
    pub fn from_log_path(log_path: &str, level: LogLevel) -> Self {
        let cur_buf = Arc::new(Mutex::new([0u8; 128]));
        let path = Path::new(log_path);
        let log_path = Arc::new(Mutex::new(path.to_path_buf()));
        Cache {
            cur_buf,
            log_path,
            vals: Arc::new(Mutex::new(HashMap::new())),
            should_exit: Arc::new(Mutex::new(false)),
            level,
        }
    }
    pub fn log_debug(&mut self, log: String) {
        match self.level { LogLevel::DEBUG => {let _ = self.write_log(log);}, _ => {}, };
    }
    pub fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Read from json in file to get cache state.

        self.log_debug("OPENING CACHE (on load) STORE".to_owned());
        let mut file = std::fs::File::open(std::env::current_dir()?.join("data/cache.json"))?;

        self.log_debug("Initializing buffer as an empty vector.".to_owned());        
        let mut buf = Vec::new();

        self.log_debug("Attempting to read from file into buffer.".to_owned());
        let bytes_read = file.read_to_end(&mut buf)?;
        self.log_debug(format!("Bytes read: {}", bytes_read));

        if buf.is_empty() {
            self.log_debug("Buffer is empty after read operation.".to_owned());
        } else {
            self.log_debug(format!("Buffer contains data: {:?}", buf));
        }

        // For every line in the file, add key then value
        self.log_debug("Handling read lines from buffer and storing in self.vals.".to_owned());
        self.vals = Arc::from(Mutex::from(self.handle_read_lines(buf)));
        self.log_debug("Contents successfully stored in self.vals.".to_owned());
        Ok(())
    }
    pub fn clean_up(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        let _ = self.write_log("OPENING CACHE (on exit) STORE".to_owned());
        let kv = self.vals.lock().unwrap();

        let len = kv.len();
        let writable_bytes = self.clone().create_byte_lines(kv);


        let _ = self.log_debug(format!("NO. OBJECTS TO STORE {}", len));

        self.log_debug(format!("ATTEMPT AMOUNT OF BYTES TO WRITE: {}", writable_bytes.len()));

        let mut file = std::fs::File::create(std::env::current_dir()?.join("data/cache.json"))?;
        let amount = file.write(&writable_bytes)?;

        self.log_debug(format!("AMOUNT OF BYTES WRITTEN: {}", amount));

        // save state to json in file (twice).
        Ok(())
    }
    pub fn handle_save (&mut self) {
        self.log_debug(format!("ATTEMPTING SAVE AT: {}", chrono::Utc::now()));
        self.clean_up().expect("Unable to clean up cache");
        self.log_debug(format!("SAVE AT: {}", chrono::Utc::now()));
    }
    pub fn create_byte_lines(&mut self, kv: MutexGuard<HashMap<[u8; 63], [u8; 64]>>) -> Vec<u8> {

        let mut bytes: Vec<u8> = Vec::new();

        self.log_debug("HANDLING BYTE LINES: Start processing byte lines.".to_owned());

        for (key, value) in kv.iter() {
            self.log_debug(format!("Processing key: {:?}", key));
            for val in key {
                bytes.push(val.clone());
            }
            self.log_debug(format!("Processed key: {:?}", key));
            self.log_debug(format!("Processed after key len: {:?}", bytes.len()));

            self.log_debug(format!("Processing value: {:?}", value));
            for val in value {
                bytes.push(val.clone());
            }
            self.log_debug(format!("Processed value: {:?}", value));
            self.log_debug(format!("Processed after value len: {:?}", bytes.len()));
        }

        self.log_debug("HANDLING BYTE LINES: Finished processing byte lines.".to_owned());
        self.log_debug(format!("SAVE BYTE LENGTH: {}", bytes.len()));

        bytes
    }

    pub fn handle_read_lines(&mut self, lines: Vec<u8>) -> HashMap<[u8; 63], [u8; 64]> {
        let mut map: HashMap<[u8; 63], [u8; 64]> = HashMap::new();

        let mut key: Vec<u8> = vec![];
        let mut value: Vec<u8> = vec![];
        let mut is_key = true;
        self.log_debug(format!("Amount of bytes found in handle_read_lines: {}", lines.len()));

        self.log_debug("Starting to process lines...".to_owned());

        for line in lines.iter() {

            //self.log_debug(format!("Key Size: {}", key.clone().len()));
            //self.log_debug(format!("Value Size: {}", value.clone().len()));

            if key.len() == 63 {
                if is_key {
                    is_key = false;
                    self.log_debug(format!("Switching to value"));
                    self.log_debug(format!("Found key: {}", std::string::String::from_utf8(key.clone()).unwrap()));
                };
            } else {
                if !is_key {
                    is_key = true;
                    self.log_debug(format!("Switching to key"));
                };
            };


            match is_key {
                true => { 
                    key.push(*line);
                },
                false => { 
                    value.push(*line);
                },
            };

            if (key.len() == 63) && (value.len() == 64) {
                self.log_debug("Appending key and value to map.".to_owned());

                let mut key_b = [0u8; 63];
                let mut value_b = [0u8; 64];
                let key_len = 63;
                let value_len = 64;

                key_b.copy_from_slice(&key[..key_len]);
                value_b.copy_from_slice(&value[..value_len]);

                map.insert(key_b, value_b);
                key.clear();
                value.clear();
                continue
            };
        };

        if map.is_empty() {
            self.log_debug("No data found!".to_owned());
        } else {
            self.log_debug(format!("Data found, size: {}", map.len()));
        }

        return map;
    }
}
