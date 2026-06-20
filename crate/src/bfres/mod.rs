mod common;
mod dict;
pub mod load;
mod save;
mod types;

pub use common::BfresError;

pub use types::Model;

use indexmap::IndexMap;
use std::collections::HashMap;
use std::io::{self, Cursor, ErrorKind, Write};

use byteorder::{LittleEndian, WriteBytesExt};

use types::ResFileData;

/// Parsed Switch BFRES container.
#[derive(Debug, Clone)]
pub struct ResFile {
    inner: ResFileData,
}

impl ResFile {
    pub fn load(data: Vec<u8>) -> io::Result<Self> {
        if data.len() < 4 || &data[0..4] != b"FRES" {
            return Err(io::Error::new(ErrorKind::InvalidData, "expected FRES magic"));
        }
        Ok(Self {
            inner: load::load_from_bytes(&data)?,
        })
    }

    pub fn save(&self) -> io::Result<Vec<u8>> {
        Ok(save::save_to_bytes(&self.inner))
    }

    pub fn canonicalize(data: &[u8]) -> io::Result<Vec<u8>> {
        let file = Self::load(data.to_vec())?;
        file.save()
    }

    /// Return attribute-name indices for the first model, used when rebuilding primitive descriptors.
    pub fn first_model_attribute_indices(data: &[u8]) -> io::Result<HashMap<String, i8>> {
        let file = Self::load(data.to_vec())?;
        Self::attribute_indices_from_model(file.first_model())
    }

    pub(crate) fn attribute_indices_from_model(model: Option<&Model>) -> io::Result<HashMap<String, i8>> {
        let mut indices = HashMap::new();
        if let Some(model) = model {
            if let Some(vertex_buffer) = model.vertex_buffers.first() {
                for (index, name) in vertex_buffer.attributes.keys().enumerate() {
                    indices.insert(name.clone(), index as i8);
                }
            }
        }
        Ok(indices)
    }

    /// Parse a single-model export once for pool rebuild.
    pub fn parse_model_export(data: Vec<u8>) -> io::Result<(String, Model)> {
        let file = load::load_from_bytes(&data).map_err(io::Error::from)?;
        let (name, model) = file
            .models
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "BFRES export has no models"))?;
        Ok((name, model))
    }

    fn first_model(&self) -> Option<&Model> {
        self.inner.models.values().next()
    }

    /// Merge single-model BFRES exports into one container (matches C# ResFile.Save after AddPrimitive).
    pub fn merge_model_files(files: &[Vec<u8>]) -> io::Result<Vec<u8>> {
        let mut merged: Option<ResFileData> = None;
        for data in files {
            let file = load::load_from_bytes(data).map_err(io::Error::from)?;
            match &mut merged {
                None => merged = Some(file),
                Some(existing) => {
                    for (name, model) in file.models {
                        existing.models.insert(name, model);
                    }
                }
            }
        }
        Ok(match merged {
            Some(file) => save::save_to_bytes(&file),
            None => Vec::new(),
        })
    }

    /// Repopulate a Base.ptcl BFRES container from per-emitter exports (matches C# AddPrimitive + Save).
    pub fn rebuild_from_base_and_exports(base: &[u8], exports: &[Vec<u8>]) -> io::Result<Vec<u8>> {
        if exports.is_empty() {
            return Ok(Vec::new());
        }
        let mut models = Vec::with_capacity(exports.len());
        for data in exports {
            models.push(Self::parse_model_export(data.clone())?);
        }
        Self::rebuild_from_base_and_models(base, None, &models)
    }

    /// Like [`rebuild_from_base_and_exports`] but reuses models parsed at load time.
    pub fn rebuild_from_base_and_models(
        base: &[u8],
        empty_base_shell: Option<&[u8]>,
        exports: &[(String, Model)],
    ) -> io::Result<Vec<u8>> {
        if exports.is_empty() {
            return Ok(Vec::new());
        }
        let mut merged = if !base.is_empty() {
            load::load_from_bytes(base).map_err(io::Error::from)?
        } else {
            let shell = empty_base_shell.ok_or_else(|| {
                io::Error::new(ErrorKind::InvalidData, "missing BFRES shell for empty base")
            })?;
            load::load_from_bytes(shell).map_err(io::Error::from)?
        };
        merged.models.clear();
        for (name, model) in exports {
            merged.models.insert(name.clone(), model.clone());
        }
        Ok(save::save_to_bytes(&merged))
    }

    pub fn export_single_model(source: &[u8], model_index: usize) -> io::Result<Vec<u8>> {
        export_single_model(source, model_index)
    }

    pub fn as_bytes(&self) -> io::Result<Vec<u8>> {
        self.save()
    }
}

/// Find the descriptor table index for a primitive ID.
pub fn descriptor_index_for_id(
    descriptors: &[crate::ptcl_file::PrimitiveDescriptor],
    id: u64,
) -> Option<usize> {
    if id == 0 || id == u64::MAX {
        return None;
    }
    descriptors.iter().position(|descriptor| descriptor.id == id)
}

/// Export one model from an embedded multi-model BFRES blob.
pub fn export_single_model(source: &[u8], model_index: usize) -> io::Result<Vec<u8>> {
    export_single_model_with_session(&mut None, source, model_index)
}

/// Export one model, reusing a session when exporting multiple models from the same blob.
pub fn export_single_model_with_session<'a>(
    session: &mut Option<load::ResExportSession<'a>>,
    source: &'a [u8],
    model_index: usize,
) -> io::Result<Vec<u8>> {
    let needs_new_session = match session {
        Some(existing) => existing.ctx.reader.data.as_ptr() != source.as_ptr(),
        None => true,
    };
    if needs_new_session {
        *session = Some(load::ResExportSession::open(source).map_err(io::Error::from)?);
    }

    let (_, model_name, model) = session
        .as_mut()
        .expect("session initialized")
        .export_model(model_index)
        .map_err(io::Error::from)?;

    let mut output = ResFileData {
        name: model_name.clone(),
        version_major: 5,
        version_minor: 0,
        version_minor2: 3,
        alignment: 0x0C,
        flag: 0,
        block_offset: 0,
        target_address_size: 0,
        external_flag: 0,
        reserve10: 0,
        data_alignment_override: 0,
        models: IndexMap::new(),
        external_files: IndexMap::new(),
        string_table_strings: Vec::new(),
        preserve_shader_param_headers: false,
    };
    output.models.insert(model.name.clone(), model);
    Ok(save::save_to_bytes(&output))
}


/// Legacy helper kept for existing tests/API surface.
pub struct BfresFile {
    magic: String,
    version: u32,
    models: Vec<(String, Vec<u8>)>,
}

impl BfresFile {
    pub fn new(magic: &str, version: u32) -> Self {
        Self {
            magic: magic.to_string(),
            version,
            models: Vec::new(),
        }
    }

    pub fn add_model(&mut self, name: &str, data: Vec<u8>) -> &mut Self {
        self.models.push((name.to_string(), data));
        self
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());

        write!(cursor, "{}", self.magic).unwrap();
        cursor.write_u32::<LittleEndian>(self.version).unwrap();

        let num_models = self.models.len() as u32;
        cursor.write_u32::<LittleEndian>(num_models).unwrap();

        for (name, data) in &self.models {
            let name_len = name.len() as u16;
            cursor.write_u16::<LittleEndian>(name_len).unwrap();
            cursor.write_all(name.as_bytes()).unwrap();

            if name_len % 2 != 0 {
                cursor.write_u8(0).unwrap();
            }

            let data_size = data.len() as u32;
            cursor.write_u32::<LittleEndian>(data_size).unwrap();

            if name_len % 2 != 0 {
                cursor.write_u8(0).unwrap();
            }

            cursor.write_all(data).unwrap();

            let current_pos = cursor.position() as usize;
            if current_pos % 16 != 0 {
                let padding_needed = 16 - (current_pos % 16);
                for _ in 0..padding_needed {
                    cursor.write_u8(0).unwrap();
                }
            }
        }

        let file_size = cursor.position() as u32;
        cursor.set_position(4);
        cursor.write_u32::<LittleEndian>(file_size).unwrap();

        cursor.into_inner()
    }
}

