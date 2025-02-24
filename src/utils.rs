use std::{collections::HashMap, sync::MutexGuard};

pub fn create_byte_lines(kv: MutexGuard<HashMap<[u8; 63], [u8; 64]>>) -> Vec<u8> {

    let mut bytes: Vec<u8> = Vec::new();

    for vals in kv.iter() {
        for val in vals.0.as_slice() {
            bytes.push(*val);
        }
        bytes.push(0x0A);

        for val in vals.1.as_slice() {
            bytes.push(*val);
        }
        bytes.push(0x0A);
    };

    bytes
}

pub fn handle_read_lines(lines: Vec<u8>) -> HashMap<[u8; 63], [u8; 64]> {
    let mut map: HashMap<[u8; 63], [u8; 64]> = HashMap::new();
    
    let mut key: Vec<u8> = vec![];
    let mut value: Vec<u8> = vec![];
    let mut is_key = true;

    for line in lines.iter() {

        // iterate until newline
        if *line == 0x0A {

            // handle edge case if any is empty
            if key.is_empty() && is_key {
                continue
            }
            
            // append both key and value if value is switched
            if !key.is_empty() && !value.is_empty() && !is_key {
                let mut key_b = [0u8; 63];
                let mut value_b = [0u8; 64];

                key_b.copy_from_slice(&key);
                value_b.copy_from_slice(&value);

                map.insert(key_b, value_b);
                key.clear();
                value.clear();
            }

            is_key = !is_key;
            continue
        }

        // Add values
        match is_key {
            true => {key.push(*line);},
            false => {value.push(*line);},
        };
    };
    return map;
}
