use std::{collections::HashMap, error, io::{self, Read}, path::{Path, PathBuf}, sync::{Arc, Mutex}};

pub mod logger;

struct Cache {
    cur_buf: [u8; 128],
    log_path: Arc<Mutex<PathBuf>>,
    vals: Arc<Mutex<HashMap<[u8;62], [u8;62]>>>
}

impl Cache {
    fn from_log_path(log_path: &str) -> Self {
        let cur_buf = [0u8; 128];
        let path = Path::new(log_path);
        let log_path = Arc::new(Mutex::new(path.to_path_buf()));
        Cache {
            cur_buf,
            log_path,
            vals: Arc::new(Mutex::new(HashMap::new()))
        }
    }
}


trait BufferAccess {
    fn read() -> Result<[u8; 128], Box<dyn std::error::Error>>;
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    fn get(key: [u8;62], map: HashMap<[u8; 62], [u8;62]>) -> Result<[u8; 62], Box<dyn std::error::Error>>;
    fn set(key: [u8;62], value: [u8;62], map: HashMap<[u8; 62], [u8;62]>) -> Result<[u8; 62], Box<dyn std::error::Error>>;
    fn remove(key: [u8;62], map: HashMap<[u8; 62], [u8;62]>) -> Result<[u8; 62], Box<dyn std::error::Error>>;
}

impl BufferAccess for Cache {
    fn read() -> Result<[u8; 128], Box<dyn std::error::Error>> {
        let mut input_buf = [0u8; 128]; // Defined length 128 chars
        io::stdin().read(&mut input_buf).unwrap();
        Ok(input_buf)
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let buf = Cache::read()?;
        self.cur_buf = buf;
        Ok(())
    }

    fn get(key: [u8;62], map: HashMap<[u8; 62], [u8;62]>) -> Result<[u8; 62], Box<dyn std::error::Error>> {
        if let Some(out) = map.get(&key) {
            return Ok(*out);
        }
        Err(Box::<dyn std::error::Error>::from("No value"))

    }

    fn set(key: [u8;62], value: [u8;62], map: HashMap<[u8; 62], [u8;62]>) -> Result<[u8; 62], Box<dyn std::error::Error>> {
        let Some(oldVal) = map.insert(key, value);
        Ok()
    }
    
}


fn main() {

    

}
