use std::{collections::HashMap, io::{Read, Write}, path::{Path, PathBuf}, process::exit, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};

use logger::{Log, Logger};
use signal_hook::{consts::{SIGHUP, SIGINT, SIGTERM}, iterator::Signals};

/*
    Just some notes
    Basically it works by accepting a buffer of 128 chars
    the first is the command
    the next 63 are the key
    the next 60 are the value
    the last 4 of the value are the total hours it is valid

    FORMAT
    stdin = Gkey0..0value0..00010

    methodkeyvaluehours_expire
 */

pub mod logger;
pub mod buffer;
pub mod tasks;

pub struct Cache {
    cur_buf: Arc<Mutex<[u8; 128]>>, // Sync update e.g. 
    log_path: Arc<Mutex<PathBuf>>, // Async logging
    vals: Arc<Mutex<HashMap<[u8;63], [u8;64]>>>, // Async KV management
    should_exit: Arc<Mutex<bool>> // Async KV management
}

impl Cache {
    fn from_log_path(log_path: &str) -> Self {
        let cur_buf = Arc::new(Mutex::new([0u8; 128]));
        let path = Path::new(log_path);
        let log_path = Arc::new(Mutex::new(path.to_path_buf()));
        Cache {
            cur_buf,
            log_path,
            vals: Arc::new(Mutex::new(HashMap::new())),
            should_exit: Arc::new(Mutex::new(false)),
        }
    }
    fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Read from json in file to get cache state.
        
        let cwd = std::env::current_dir()?.join("data/cache.json");
        let mut file = std::fs::File::open(cwd)?;
        let mut buf = vec![];
        file.read(&mut buf)?;
        let stringified = std::string::String::from_utf8(buf)?;
        
        let deserialized: HashMap<String,String> = serde_json::from_str(&stringified)?;
        let mut kv = self.vals.lock().unwrap();

        for val in deserialized {

            let mut key_b = [0u8; 63];  // Initialize buffer for key
            let mut value_b = [0u8; 64];  // Initialize buffer for value

            let key = val.0.as_bytes();
            let value = val.1.as_bytes();
            println!("key: {} keylen: {}", std::str::from_utf8(&key).unwrap(), key.len());

            // Fill the key buffer, ensuring it doesn't panic if key is smaller than the buffer
            let key_len = key.len().min(key_b.len());  // Get the minimum of key length and buffer size
            key_b[..key_len].clone_from_slice(&key[..key_len]);  // Fill the buffer with the key

            // If the key is shorter, fill the rest of the buffer with a specific byte (e.g., 0xD1)
            if key_len < key_b.len() {
                key_b[key_len..].fill(0x00);
            }

            // Fill the value buffer, ensuring it doesn't panic if value is smaller than the buffer
            let value_len = value.len().min(value_b.len());  // Get the minimum of value length and buffer size
            value_b[..value_len].clone_from_slice(&value[..value_len]);  // Fill the buffer with the value

            // If the value is shorter, fill the rest of the buffer with a specific byte (e.g., 0xD1)
            if value_len < value_b.len() {
                value_b[value_len..].fill(0x00);
            }
            
            kv.insert(key_b, value_b).unwrap();
        }

        Ok(())
    }
    fn clean_up(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        
        let cwd = std::env::current_dir()?.join("data/cache.json");
        let _ = self.write_log(format!("OPENING {}", cwd.to_str().unwrap()));
        let kv = self.vals.lock().unwrap();

        let mut serializable: HashMap<String,String> = HashMap::new();

        for val in kv.iter() {
            serializable.insert(String::from_utf8_lossy(val.0).into_owned(), String::from_utf8_lossy(val.1).into_owned());
        }

        let json = match serde_json::to_string(&serializable) {
            Ok(v) => {v},
            Err(e) => {println!("Error formatting json");return Err(Box::new(e));}
        };
        let mut file = std::fs::File::create(cwd)?;
        file.write(json.as_bytes())?;

        // save state to json in file (twice).
        Ok(())
    }
}

fn handle_close (cache: Arc<Mutex<Cache>>) {
    println!("Received Ctrl+C (SIGINT)! Cleaning up...");
    let mut cache = cache.lock().unwrap();
    cache.write_log(format!("ATTEMPTING EXIT AT: {}", chrono::Utc::now())).unwrap();
    cache.clean_up().expect("Unable to clean up cache");
    cache.write_log(format!("EXIT AT: {}", chrono::Utc::now())).unwrap();

    std::process::exit(0);
}

fn main() {
    let mut cache = Cache::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log");
    match cache.load() {
        Ok(_) => {},
        _ => {},
    };

    let mut time = format!("START AT SYSTEMTIME: {}", chrono::Utc::now());
    let logger = Logger::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log", false);
    let _ = cache.write_log(&mut time);

    let cache = Arc::from(Mutex::from(cache));
    let running = Arc::new(AtomicBool::new(true));
    let running_listener = Arc::clone(&running);

    let mut signals = Signals::new(&[SIGINT, SIGHUP, SIGTERM]).expect("Cannot set signal handlers");

    let cache_for_cleanup = Arc::clone(&cache);

    let handle = std::thread::spawn(move || {
            for signal in signals.forever() {
                match signal {
                    SIGINT => handle_close(cache_for_cleanup),
                    SIGHUP => handle_close(cache_for_cleanup),
                    SIGTERM => handle_close(cache_for_cleanup),
                    _ => {}
                }
            running_listener.store(false, Ordering::SeqCst);
            break; // Exit the loop after handling the signal
        }
    });

    while running.load(Ordering::SeqCst) {
        let _ = tasks::run_tasks(&cache, Arc::from(Mutex::from(logger.clone())));
    }

    handle.join().unwrap();
}
