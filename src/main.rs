use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex}};

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
}

fn main() {
    let mut cache = Cache::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log");
    let mut time = format!("START AT SYSTEMTIME: {}", chrono::Utc::now());
    let logger = Logger::from_log_path("/Users/elliothegraeus/Documents/BASE/projects/cacherebook/log/log.log", false);
    let _ = cache.write_log(&mut time);
    let _ = tasks::run_tasks(Arc::from(Mutex::from(cache)), Arc::from(Mutex::from(logger)));
}
