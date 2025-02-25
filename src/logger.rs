use std::{fmt::{self, Display}, fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}, sync::{Arc, Mutex, RwLock}};
use chrono::Utc;
use crate::Cache;

#[derive(Clone, Debug)]
pub struct Logger {
    pub log_path: Arc<RwLock<PathBuf>>,
    buffer: Arc<Mutex<Vec<u8>>>,
    out: bool,
    buffer_size: usize,
}

impl Logger {
    pub fn new(log_path: &str, out: bool) -> Self {
        let path = Path::new(log_path);
        let log_path = Arc::new(RwLock::new(path.to_path_buf()));
        
        // Initialize the file if it doesn't exist
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(path, b"").ok();
        }
        
        // Create buffer with optimized initial capacity
        let buffer = Arc::new(Mutex::new(Vec::with_capacity(8192)));
        
        Logger {
            log_path,
            buffer,
            out,
            buffer_size: 8192, // 8KB buffer
        }
    }
    
    // Flush the log buffer to disk, improving I/O efficiency
    pub fn flush(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = self.buffer.lock().unwrap();
        if buffer.is_empty() {
            return Ok(());
        }
        
        let path = self.log_path.read().unwrap();
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&*path)?;
        
        file.write_all(&buffer)?;
        buffer.clear();
        
        Ok(())
    }
}

pub trait Log<T>
where T: Display
{
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>>;
}

impl<T> Log<T> for Logger
where
    T: Display + fmt::Write + std::marker::Send + std::marker::Sync,
{
    // Optimized to use a buffer and minimize file I/O
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>> 
        where T: std::fmt::Display + Send,
    {
        let input_string = input.to_string();
        
        // Use Cow for zero-copy when possible
        let log_entry = format!("\n\r[LOG@{}]{}", Utc::now(), input_string);
        
        // Add to buffer
        {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.extend_from_slice(log_entry.as_bytes());
            
            // Flush if buffer exceeds threshold
            if buffer.len() >= self.buffer_size {
                let path = self.log_path.read().unwrap();
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&*path)?;
                file.write_all(&buffer)?;
                buffer.clear();
            }
        }
        
        if self.out {
            println!("{}", input_string);
        }
        
        Ok(())
    }
}

impl<T> Log<T> for Cache
where
    T: Display + fmt::Write + std::marker::Send + std::marker::Sync,
{
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>> 
        where T: std::fmt::Display + Send,
    {
        let input_clone = input.to_string();
        
        if let Some(logger) = &mut self.logger {
            logger.write_log(input_clone)?;
        } else {
            // Fallback if logger not initialized
            if self.level == crate::LogLevel::DEBUG {
                println!("[LOG] {}", input_clone);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_log_success() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let path = temp_file.path().to_str().unwrap().to_string();
        let mut logger = Logger::new(&path, false);
        
        let input = "Test message".to_string();
        
        // Call write_log
        let result = logger.write_log(input);
        logger.flush().unwrap();
        
        assert!(result.is_ok(), "write_log failed: {:?}", result.err());
        
        // Read the file contents
        let contents = fs::read_to_string(temp_file.path()).expect("Failed to read temp file");
        assert!(contents.contains("Test message"));
    }

    #[test]
    fn test_write_log_buffering() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let path = temp_file.path().to_str().unwrap().to_string();
        let mut logger = Logger::new(&path, false);
        
        // Set small buffer size for testing
        logger.buffer_size = 100;
        
        // Write multiple messages
        for i in 0..20 {
            let input = format!("Message {}", i);
            logger.write_log(input).expect("Write failed");
        }
        
        // Flush remaining messages
        logger.flush().unwrap();
        
        // Read the file contents
        let contents = fs::read_to_string(temp_file.path()).expect("Failed to read temp file");
        
        // Verify all messages were written
        for i in 0..20 {
            assert!(contents.contains(&format!("Message {}", i)));
        }
    }
}


