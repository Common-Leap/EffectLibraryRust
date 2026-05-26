use std::io::{Cursor, Write};
use byteorder::{LittleEndian, WriteBytesExt};

/// Represents a BFRES (Binary File Resource) file structure.
pub struct BfresFile {
    magic: String,
    version: u32,
    models: Vec<(String, Vec<u8>)>,
}

impl BfresFile {
    /// Creates a new BFRES file with the given magic and version.
    pub fn new(magic: &str, version: u32) -> Self {
        BfresFile {
            magic: magic.to_string(),
            version,
            models: Vec::new(),
        }
    }

    /// Adds a model to the BFRES file.
    pub fn add_model(&mut self, name: &str, data: Vec<u8>) -> &mut Self {
        self.models.push((name.to_string(), data));
        self
    }

    /// Serializes the BFRES file into binary format based on Wii U resource format specification.
    pub fn serialize(&self) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());

        // Write magic (BFRES)
        write!(cursor, "{}", self.magic).unwrap();

        // Write version (0x10 for Wii U resources)
        cursor.write_u32::<LittleEndian>(self.version).unwrap();

        // Write number of models
        let num_models = self.models.len() as u32;
        cursor.write_u32::<LittleEndian>(num_models).unwrap();

        // Write each model with proper BFRES structure
        for (name, data) in &self.models {
            // Write model name length and name (UTF-8)
            let name_len = name.len() as u16;
            cursor.write_u16::<LittleEndian>(name_len).unwrap();
            cursor.write_all(name.as_bytes()).unwrap();

            // Write padding to align to 4 bytes
            if name_len % 2 != 0 {
                cursor.write_u8(0).unwrap();
            }

            // Write model binary size (aligned)
            let data_size = data.len() as u32;
            cursor.write_u32::<LittleEndian>(data_size).unwrap();

            // Write padding to align to 4 bytes
            if name_len % 2 != 0 {
                cursor.write_u8(0).unwrap();
            }

            // Write model binary data
            cursor.write_all(data).unwrap();

            // Write padding to align to 16 bytes (Wii U resource alignment)
            let current_pos = cursor.position() as usize;
            if current_pos % 16 != 0 {
                let padding_needed = 16 - (current_pos % 16);
                for _ in 0..padding_needed {
                    cursor.write_u8(0).unwrap();
                }
            }
        }

        // Write file size at the end
        let file_size = cursor.position() as u32;
        cursor.set_position(4); // Seek to position after version
        cursor.write_u32::<LittleEndian>(file_size).unwrap();

        cursor.into_inner()
    }
}
