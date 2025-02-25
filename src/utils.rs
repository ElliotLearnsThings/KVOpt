
use std::{borrow::Cow, fs, io::{self, BufWriter, Write}, path::Path};

use chrono::{DateTime, TimeDelta, Utc};

use crate::CacheEntry;

// Utility function to extract timestamp and expiration from cache value
pub fn parse_cache_metadata(value: &[u8; 64]) -> (DateTime<Utc>, Option<DateTime<Utc>>) {
    // Get timestamp (6 bytes)
    let mut timestamp_bytes = [0u8; 8];
    timestamp_bytes[2..8].copy_from_slice(&value[56..62]);
    let timestamp = i64::from_be_bytes(timestamp_bytes);
    
    // Get expiration (2 bytes)
    let mut expiry_bytes = [0u8; 2];
    expiry_bytes.copy_from_slice(&value[62..64]);
    let expiry_seconds = u16::from_be_bytes(expiry_bytes);
    
    // Create DateTime objects
    let created_at = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_else(|| Utc::now());
    
    let expires_at = if expiry_seconds > 0 {
        Some(created_at + TimeDelta::try_seconds(expiry_seconds as i64).unwrap_or_default())
    } else {
        None
    };
    
    (created_at, expires_at)
}

// Utility function to create a cache entry from raw data
pub fn create_cache_entry(value: &[u8; 64]) -> CacheEntry {
    let mut value_only = [0u8; 56];
    value_only.copy_from_slice(&value[0..56]);
    
    let (created_at, expires_at) = parse_cache_metadata(value);
    
    CacheEntry {
        value: value_only,
        created_at,
        expires_at,
    }
}

// Efficiently write a large buffer to a file
pub fn write_buffer_to_file(path: &Path, buffer: &[u8]) -> io::Result<usize> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Use BufWriter for efficient writing
    let file = fs::File::create(path)?;
    let mut writer = BufWriter::with_capacity(65536, file); // 64KB buffer
    
    let bytes_written = writer.write(buffer)?;
    writer.flush()?;
    
    Ok(bytes_written)
}

// Efficiently read a file into a buffer
pub fn read_file_to_buffer(path: &Path) -> io::Result<Vec<u8>> {
    // Check if file exists first
    if !path.exists() {
        return Ok(Vec::new());
    }
    
    // Get file size for pre-allocation
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len() as usize;
    
    // Pre-allocate buffer
    let mut buffer = Vec::with_capacity(file_size);
    let file = fs::File::open(path)?;
    
    // Use Read trait to fill buffer
    io::Read::read_to_end(&mut io::BufReader::new(file), &mut buffer)?;
    
    Ok(buffer)
}

// Convert raw bytes to a string, handling non-UTF8 data safely
pub fn bytes_to_string(bytes: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

// Helper function to get current timestamp in seconds since epoch
pub fn current_timestamp() -> i64 {
    Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_parse_cache_metadata() {
        // Create a test value with known timestamp and expiration
        let mut value = [0u8; 64];
        
        // Set timestamp to a known value (2023-01-01 00:00:00 UTC = 1672531200)
        let timestamp = 1672531200i64;
        let timestamp_bytes = timestamp.to_be_bytes();
        value[56..62].copy_from_slice(&timestamp_bytes[2..8]);
        
        // Set expiration to 3600 seconds (1 hour)
        let expiry = 3600u16;
        let expiry_bytes = expiry.to_be_bytes();
        value[62..64].copy_from_slice(&expiry_bytes);
        
        // Parse metadata
        let (created_at, expires_at) = parse_cache_metadata(&value);
        
        // Check created_at
        assert_eq!(created_at.timestamp(), timestamp);
        
        // Check expires_at
        assert!(expires_at.is_some());
        if let Some(expires) = expires_at {
            assert_eq!(expires.timestamp(), timestamp + expiry as i64);
        }
    }
    
    #[test]
    fn test_create_cache_entry() {
        // Create a test value
        let mut value = [0u8; 64];
        
        // Set some data in the value portion
        value[0..5].copy_from_slice(b"hello");
        
        // Set timestamp and expiration
        let timestamp = Utc::now().timestamp();
        let timestamp_bytes = timestamp.to_be_bytes();
        value[56..62].copy_from_slice(&timestamp_bytes[2..8]);
        
        let expiry = 3600u16;
        let expiry_bytes = expiry.to_be_bytes();
        value[62..64].copy_from_slice(&expiry_bytes);
        
        // Create cache entry
        let entry = create_cache_entry(&value);
        
        // Check value portion
        assert_eq!(&entry.value[0..5], b"hello");
        
        // Check metadata
        assert!(entry.expires_at.is_some());
    }
    
    #[test]
    fn test_write_read_buffer() {
        // Create a temp file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        
        // Create test data
        let data = b"Hello, world!".repeat(1000);
        
        // Write data
        let bytes_written = write_buffer_to_file(path, &data).unwrap();
        assert_eq!(bytes_written, data.len());
        
        // Read data back
        let read_data = read_file_to_buffer(path).unwrap();
        
        // Verify
        assert_eq!(read_data, data);
    }
}


