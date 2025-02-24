use std::{collections::HashMap, io::{Read, Write}, path::{Path, PathBuf}, process::exit, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};

use logger::{Log, Logger};

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
pub mod utils;


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
        
        let mut file = std::fs::File::open(std::env::current_dir()?.join("data/cache.json"))?;
        let mut buf = vec![];
        file.read(&mut buf)?;

        // For every line in the file, add key then value
        self.vals = Arc::from(Mutex::from(utils::handle_read_lines(buf)));
        Ok(())
    }
    fn clean_up(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        
        let _ = self.write_log("OPENING CACHE STORE".to_owned());
        let kv = self.vals.lock().unwrap();

        let write_bytes = utils::create_byte_lines(kv);

        let mut file = std::fs::File::create(std::env::current_dir()?.join("data/cache.json"))?;
        file.write(&write_bytes)?;

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
}

#[cfg(windows)]
fn main() {
    use chrono::Utc;

    let cwd = std::env::current_dir().unwrap();
    let log_dir = cwd.join("log/log.log");

    let log_dir = match log_dir.to_str() {
        Some(v) => {v},
        _ => panic!("Could not resolve log path!")
    };

    let mut cache = Cache::from_log_path(&log_dir);
    match cache.load() {
        Ok(_) => {},
        _ => {},
    };

    let mut time = format!("START AT SYSTEMTIME: {}", Utc::now());
    let logger = Logger::from_log_path(log_dir, false);
    let _ = cache.write_log(&mut time);

    let cache = Arc::from(Mutex::from(cache));
    let running = Arc::new(AtomicBool::new(true));
    
    let running_listener = Arc::clone(&running);
    let cache_for_cleanup = Arc::clone(&cache);

    // Set up signal handler using ctrc crate
    ctrlc::set_handler(move || {
        running_listener.store(false, Ordering::SeqCst);
        handle_close(cache_for_cleanup.clone());
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");

    // Main loop to run tasks
    while running.load(Ordering::SeqCst) {
        let _ = tasks::run_tasks(&cache, Arc::from(Mutex::from(logger.clone())));
    }
}

#[cfg(unix)]
use signal_hook::{consts::{SIGHUP, SIGINT, SIGTERM}, iterator::Signals};
#[cfg(unix)]
fn main() {
    let cwd = std::env::current_dir().unwrap();
    let log_dir = cwd.join("log/log.log");

    let log_dir = match log_dir.to_str() {
        Some(v) => {v},
        _ => panic!("Could not resolve log path!")
    };

    let mut cache = Cache::from_log_path(log_dir);
    match cache.load() {
        Ok(_) => {},
        _ => {},
    };

    let mut time = format!("START AT SYSTEMTIME: {}", chrono::Utc::now());
    let logger = Logger::from_log_path(log_dir, false);
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
