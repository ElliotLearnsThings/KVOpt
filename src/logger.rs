use core::fmt;
use std::{fmt::Display, path::{Path, PathBuf}, sync::{Arc, Mutex}};

use crate::Cache;

#[derive(Clone)]
pub struct Logger {
    pub log_path: Arc<Mutex<PathBuf>>,
    out: bool,
}

impl Logger {
    pub fn from_log_path(log_path: &str, out: bool) -> Self {
        let path = Path::new(log_path);
        let log_path = Arc::new(Mutex::new(path.to_path_buf()));
        Logger {
            log_path,
            out,
        }
    }
}

impl<T> Log<T> for Logger
where
    T: Display + fmt::Write + std::marker::Send + std::marker::Sync,
{
    // This will be on spawned thread
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>> 
        where T: std::fmt::Display + Send,
    {

        let input_clone = input.to_string();
        let path_name = Arc::clone(&self.log_path);
        let out = self.out.clone();

        let log_thread = std::thread::spawn({
            move || {
                let path = path_name.lock().unwrap();
                let mut buf = std::fs::read(&*path).unwrap();
                let log_entry = format!("\n\r[LOG]{}", input_clone);
                buf.extend_from_slice(log_entry.as_bytes());
                std::fs::write(&*path, &buf).unwrap();
            }
        });

        if out {
            println!("{}", input);
        }

        log_thread.join().unwrap();
        Ok(())
    }
}

pub trait Log <T>
where T: Display
{
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>>;
}

impl<T> Log<T> for Cache
where
    T: Display + fmt::Write + std::marker::Send + std::marker::Sync,
{
    // This will be on spawned thread
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>> 
        where T: std::fmt::Display + Send,
    {

        let input_clone = input.to_string();
        let path_name = Arc::clone(&self.log_path);

        let log_thread = std::thread::spawn({
            move || {
                let path = path_name.lock().unwrap();
                let mut buf = std::fs::read(&*path).unwrap();
                let log_entry = format!("\n\r[LOG]{}", input_clone);
                buf.extend_from_slice(log_entry.as_bytes());
                std::fs::write(&*path, &buf).unwrap();
            }
        });

        log_thread.join().unwrap();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::Mutex};
    use tempfile::NamedTempFile;

    // Helper to create a Cache with a temp file
    fn setup_cache() -> (Cache, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let path = temp_file.path().to_str().unwrap().to_string();
        let cache = Cache::from_log_path(&path);
        (cache, temp_file)
    }

    #[test]
    fn test_write_log_success() {
        let (mut cache, temp_file) = setup_cache();
        let input = format!("Test message");

        // Call write_log
        let result = cache.write_log(input);
        assert!(result.is_ok(), "write_log failed: {:?}", result.err());

        // Read the file contents
        let contents = fs::read_to_string(temp_file.path()).expect("Failed to read temp file");
        assert_eq!(contents, "\n\r[LOG]Test message");
    }

    #[test]
    fn test_write_log_multiple_calls() {
        let (mut cache, temp_file) = setup_cache();
        let input1 = format!("First message");
        let input2 = format!("Second message");

        // Write twice
        cache.write_log(input1).expect("First write failed");
        cache.write_log(input2).expect("Second write failed");

        // Check contents
        let contents = fs::read_to_string(temp_file.path()).expect("Failed to read temp file");
        assert_eq!(contents, "\n\r[LOG]First message\n\r[LOG]Second message");
    }

    #[test]
    fn test_write_log_empty_input() {
        let (mut cache, temp_file) = setup_cache();
        let input = format!("");

        let result = cache.write_log(input);
        assert!(result.is_ok(), "write_log failed with empty input: {:?}", result.err());

        let contents = fs::read_to_string(temp_file.path()).expect("Failed to read temp file");
        assert_eq!(contents, "\n\r[LOG]");
    }

    // This test assumes the file is writable; testing unwritable paths is trickier
    #[test]
    fn test_write_log_thread_safety() {
        let (cache, _temp_file) = setup_cache();
        let cache = Arc::new(Mutex::new(cache));
        let cache_clone = Arc::clone(&cache);

        // Spawn a thread to write concurrently
        let handle = std::thread::spawn(move || {
            let mut cache = cache_clone.lock().unwrap();
            cache.write_log(format!("Thread message")).unwrap();
        });

        // Write from main thread
        cache.lock().unwrap().write_log(format!("Main message")).unwrap();
        handle.join().unwrap();

        // Check that both messages are present (order may vary due to threading)
        let contents = fs::read_to_string(_temp_file.path()).expect("Failed to read temp file");
        assert!(contents.contains("[LOG]Thread message"));
        assert!(contents.contains("[LOG]Main message"));
    }
}
