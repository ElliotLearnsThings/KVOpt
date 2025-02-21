use core::fmt;
use std::{fmt::Display, io, sync::Arc};

use crate::Cache;

pub trait Log <T>
where T: Display
{
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>>;
}

impl<T> Log<T> for Cache
where
    T: Display + io::Write + fmt::Write + std::marker::Send + std::marker::Sync,
{
    // This will be on spawned thread
    fn write_log(&mut self, input: T) -> Result<(), Box<dyn std::error::Error>> 
        where T: std::fmt::Display + Send,
    {

        let input_clone = input.to_string();

        let log_thread = std::thread::spawn({
            let path_name = Arc::clone(&self.log_path);
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
