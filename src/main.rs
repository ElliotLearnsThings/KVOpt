use std::{io::{Read, Write}, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc, Mutex, RwLock}};
use chrono::{DateTime, TimeDelta, Utc};
use dashmap::DashMap;
use hashbrown;
use logger::{Log, Logger};

/*
    Cache format:
    - First byte: command (G=get, I=insert, R=remove, H=halt)
    - Next 63 bytes: key
    - Next 56 bytes: value
    - Last 8 bytes:
      - First 6 bytes: timestamp (epoch seconds)
      - Last 2 bytes: expiration time in seconds
 */

pub mod logger;
pub mod buffer;
pub mod tasks;
pub mod utils;

// Thread pool for background tasks
const THREAD_POOL_SIZE: usize = 4;
const DEFAULT_INVALIDATION_THRESHOLD: usize = 100;
const PERSISTENCE_INTERVAL_SECS: u64 = 10; // More frequent persistence

#[derive(Clone, Copy, PartialEq)]
pub enum LogLevel {
    NORMAL,
    DEBUG,
}

// Cache entry with metadata for more efficient expiration handling
#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub value: [u8; 56],
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub struct Cache {
    cur_buf: Arc<Mutex<[u8; 128]>>,
    log_path: Arc<RwLock<PathBuf>>,
    // Using RwLock instead of Mutex for better read concurrency
    vals: Arc<RwLock<hashbrown::HashMap<[u8;63], [u8;64]>>>,
    // Using DashMap for highly concurrent access patterns
    entries: Arc<DashMap<[u8;63], CacheEntry>>,
    should_exit: Arc<AtomicBool>,
    level: LogLevel,
    logger: Option<Logger>,
    // Track operations since last invalidation for batched invalidation
    ops_since_invalidation: Arc<AtomicUsize>,
    invalidation_threshold: usize,
    // Thread pool for background tasks
    thread_pool: Arc<threadpool::ThreadPool>,
    // Flag to indicate when to save
    pub save_flag: Arc<AtomicBool>,
    // Data directory
    data_dir: PathBuf,
}

impl Cache {
    pub fn new(log_path: &str, level: LogLevel) -> Self {
        // Create a single, reusable buffer
        let cur_buf = Arc::new(Mutex::new([0u8; 128]));
        
        // Use Path API properly
        let path = Path::new(log_path);
        let log_path = Arc::new(RwLock::new(path.to_path_buf()));
        
        // Create logger
        let logger = Some(Logger::new(log_path.as_ref().read().unwrap().to_str().unwrap(), level == LogLevel::DEBUG));
        
        // Create thread pool
        let thread_pool = Arc::new(threadpool::ThreadPool::new(THREAD_POOL_SIZE));
        
        // Set up data directory
        let data_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("data");
        
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&data_dir).ok();
        
        Cache {
            cur_buf,
            log_path,
            vals: Arc::new(RwLock::new(hashbrown::HashMap::with_capacity(10000))),
            entries: Arc::new(DashMap::with_capacity(10000)),
            should_exit: Arc::new(AtomicBool::new(false)),
            level,
            logger,
            ops_since_invalidation: Arc::new(AtomicUsize::new(0)),
            invalidation_threshold: DEFAULT_INVALIDATION_THRESHOLD,
            thread_pool,
            save_flag: Arc::new(AtomicBool::new(false)),
            data_dir,
        }
    }
    
    pub fn log_debug(&mut self, log: String) {
        if self.level == LogLevel::DEBUG {
            let _ = self.write_log(log);
        }
    }
    
    // Optimized invalidation that runs in the background
    pub fn invalidate_cache(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let entries = Arc::clone(&self.entries);
        let vals = Arc::clone(&self.vals);
        let level = self.level;
        
        // Use the thread pool for background invalidation
        let logger_clone = self.logger.clone();
        
        self.thread_pool.execute(move || {
            let mut keys_to_remove = Vec::new();
            let now = Utc::now();
            
            // Efficient iteration with DashMap
            for entry in entries.iter() {
                let key = entry.key();
                let cache_entry = entry.value();
                
                if let Some(expires_at) = cache_entry.expires_at {
                    if expires_at <= now {
                        keys_to_remove.push(*key);
                        
                        if level == LogLevel::DEBUG {
                            if let Some(mut logger) = logger_clone.clone() {
                                let _ = logger.write_log(format!(
                                    "REMOVED KEY DUE TO EXPIRATION: {:?}, EXPIRED AT: {}",
                                    key, expires_at
                                ));
                            }
                        }
                    }
                }
            }
            
            // Remove expired entries
            for key in keys_to_remove {
                entries.remove(&key);
                vals.write().unwrap().remove(&key);
            }
        });
        
        Ok(())
    }
    
    // Load cache from disk
    pub fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.log_debug("LOADING CACHE FROM DISK".to_owned());
        
        let cache_path = self.data_dir.join("cache.json");
        if !cache_path.exists() {
            self.log_debug("No cache file found, starting with empty cache".to_owned());
            return Ok(());
        }
        
        let mut file = std::fs::File::open(&cache_path)?;
        let mut buf = Vec::with_capacity(1024 * 1024); // Pre-allocate 1MB
        
        let bytes_read = file.read_to_end(&mut buf)?;
        self.log_debug(format!("Read {} bytes from cache file", bytes_read));
        
        if buf.is_empty() {
            self.log_debug("Cache file is empty".to_owned());
            return Ok(());
        }
        
        // Process the data in chunks of 127 bytes (key+value)
        let mut vals = hashbrown::HashMap::with_capacity(bytes_read / 127);
        let entries = DashMap::with_capacity(bytes_read / 127);
        
        for chunk in buf.chunks_exact(127) {
            if chunk.len() == 127 {
                let mut key = [0u8; 63];
                let mut value = [0u8; 64];
                
                key.copy_from_slice(&chunk[0..63]);
                value.copy_from_slice(&chunk[63..127]);
                
                // Skip empty keys
                if key.iter().all(|&b| b == 0) {
                    continue;
                }
                
                // Extract timestamp and expiration
                let mut timestamp_bytes = [0u8; 8];
                timestamp_bytes[2..8].copy_from_slice(&value[56..62]);
                let timestamp = i64::from_be_bytes(timestamp_bytes);
                
                let mut expiry_bytes = [0u8; 2];
                expiry_bytes.copy_from_slice(&value[62..64]);
                let expiry_seconds = u16::from_be_bytes(expiry_bytes);
                
                // Create cache entry
                let created_at = DateTime::<Utc>::from_timestamp(timestamp, 0)
                    .unwrap_or_else(|| Utc::now());
                
                let expires_at = if expiry_seconds > 0 {
                    Some(created_at + TimeDelta::try_seconds(expiry_seconds as i64).unwrap_or_default())
                } else {
                    None
                };
                
                let mut value_only = [0u8; 56];
                value_only.copy_from_slice(&value[0..56]);
                
                let entry = CacheEntry {
                    value: value_only,
                    created_at,
                    expires_at,
                };
                
                // Don't load expired entries
                if let Some(expires) = expires_at {
                    if expires <= Utc::now() {
                        self.log_debug(format!("Skipping expired entry from disk"));
                        continue;
                    }
                }
                
                vals.insert(key, value);
                entries.insert(key, entry);
            }
        }
        
        self.log_debug(format!("Loaded {} entries into cache", vals.len()));
        
        // Update the cache
        *self.vals.write().unwrap() = vals;
        self.entries = Arc::new(entries);
        
        // Run initial invalidation to clean up any expired entries
        self.invalidate_cache()?;
        
        Ok(())
    }
    
    // Save cache to disk
    pub fn clean_up(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.log_debug("SAVING CACHE TO DISK".to_owned());
        
        // Ensure the data directory exists
        if !self.data_dir.exists() {
            std::fs::create_dir_all(&self.data_dir)?;
        }
        
        let cache_path = self.data_dir.join("cache.json");
        let mut file = std::fs::File::create(&cache_path)?;
        
        // Serialize the cache efficiently
        let kv = self.vals.read().unwrap();
        let len = kv.len();
        
        // Pre-allocate buffer with exact size needed
        let mut buffer = Vec::with_capacity(len * 127);
        
        for (key, value) in kv.iter() {
            // Don't persist empty keys
            if key.iter().all(|&b| b == 0) {
                continue;
            }
            
            // Check if entry is expired before persisting
            let mut timestamp_bytes = [0u8; 8];
            timestamp_bytes[2..8].copy_from_slice(&value[56..62]);
            let timestamp = i64::from_be_bytes(timestamp_bytes);
            
            let mut expiry_bytes = [0u8; 2];
            expiry_bytes.copy_from_slice(&value[62..64]);
            let expiry_seconds = u16::from_be_bytes(expiry_bytes);
            
            if expiry_seconds > 0 {
                let created_at = DateTime::<Utc>::from_timestamp(timestamp, 0)
                    .unwrap_or_else(|| Utc::now());
                
                let expires_at = created_at + TimeDelta::try_seconds(expiry_seconds as i64).unwrap_or_default();
                
                // Skip if expired
                if expires_at <= Utc::now() {
                    continue;
                }
            }
            
            buffer.extend_from_slice(key);
            buffer.extend_from_slice(value);
        }
        
        let bytes_written = file.write(&buffer)?;
        
        // Flush to ensure data is written
        file.flush()?;
        
        // Reset save flag
        self.save_flag.store(false, Ordering::SeqCst);
        drop(kv);

        self.log_debug(format!("Wrote {} bytes to cache file", bytes_written));

        Ok(())
    }
}

fn handle_close(cache: Arc<Mutex<Cache>>) {
    println!("Received signal! Cleaning up...");
    
    // Get a lock on the cache
    if let Ok(mut cache) = cache.lock() {
        cache.log_debug("HANDLING SHUTDOWN".to_string());
        
        // Save cache to disk
        if let Err(e) = cache.clean_up() {
            eprintln!("Error during cleanup: {}", e);
        }
        
        // Set the exit flag
        cache.should_exit.store(true, Ordering::SeqCst);
        
        cache.log_debug(format!("EXIT AT: {}", Utc::now()));
    }
}

#[cfg(not(test))]
fn main() {
    // Initialize the cache
    let log_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("log/log.log");
    
    let log_path = log_dir.to_str().unwrap_or("./log/log.log");
    let mut cache = Cache::new(log_path, LogLevel::DEBUG);
    
    // Load existing cache data
    let init_time = Utc::now();
    if let Err(e) = cache.load() {
        eprintln!("Error loading cache: {}", e);
    }
    
    let final_time = Utc::now();
    let time_delta = final_time - init_time;
    
    cache.log_debug(format!("Cache initialization took {} ms", time_delta.num_milliseconds()));
    cache.log_debug(format!("START AT: {}", Utc::now()));
    
    // Wrap the cache in an Arc<Mutex<>> for thread-safe access
    let cache = Arc::new(Mutex::new(cache));
    
    // Set up signal handlers for proper cleanup
    setup_signal_handlers(Arc::clone(&cache));
    
    // Run the main task loop
    if let Err(e) = tasks::run_tasks(&cache) {
        eprintln!("Error in main task loop: {}", e);
    }
}

#[cfg(windows)]
fn setup_signal_handlers(cache: Arc<Mutex<Cache>>) {
    let cache_for_cleanup = Arc::clone(&cache);
    ctrlc::set_handler(move || {
        handle_close(cache_for_cleanup.clone());
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");
}

#[cfg(unix)]
fn setup_signal_handlers(cache: Arc<Mutex<Cache>>) {
    use signal_hook::{consts::{SIGHUP, SIGINT, SIGTERM}, iterator::Signals};
    
    let cache_for_cleanup = Arc::clone(&cache);
    
    std::thread::spawn(move || {
        let signals = Signals::new(&[SIGINT, SIGHUP, SIGTERM]).expect("Cannot set signal handlers");
        
        for signal in signals.forever() {
            match signal {
                SIGINT | SIGHUP | SIGTERM => {
                    handle_close(cache_for_cleanup.clone());
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    });
}

