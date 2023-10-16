#![allow(dead_code)]

use crate::nom_parser::{
    parser, ChunkHeader, ChunkType, PackageChunkHeader, ResValue, TableEntryFlag, TableMap,
    TableMapEntry, TypeChunkHeader,
};
use crate::nom_parser::{ParseError, TypeSpecChunkHeader};
use nom::multi::count;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Arsc<'barsc> {
    binary_arsc: &'barsc [u8],
    strings: Vec<String>,
    packages: HashMap<u32, Package>,
}

// contains resource entry values
// can contain multiple values or a single value
type ResEntry = Vec<Option<String>>;
type TypeId = u32;

#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub type_strings: Vec<String>,
    pub key_strings: Vec<String>,
    pub type_spec: Vec<(TypeSpecChunkHeader, Vec<u32>)>,
    /// `types` field in the `Package` struct.
    ///
    /// This field is a collection of tuples, where each tuple consists of a unique identifier (`TypeId`)
    /// for a resource type and a vector of `ResEntry`. Each `ResEntry` can contain multiple values or a single
    /// value, represented as an `Option<String>`. This allows for flexibility as some resources might have
    /// multiple values (for different configurations), while others might only have a single value.
    ///
    /// The `TypeId` is a unique identifier for each resource type in a package, which allows for efficient
    /// retrieval of resources based on their types.
    pub types: Vec<(TypeId, Vec<ResEntry>)>,
}

impl<'barsc> Arsc<'barsc> {
    pub fn new(binary_arsc: &'barsc [u8]) -> Self {
        Self {
            binary_arsc,
            strings: Vec::new(),
            packages: HashMap::new(),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<u8>, ParseError> {
        let (_, arsc_table_header) = parser::parse_table(self.binary_arsc)
            .map_err(|e| ParseError::ChunkHeader(e.to_string()))?;

        // Android doesn't care about the chunk type
        // chunk_header.typ != ChunkType::Table

        // table header size: 8 + 4 => 12
        let mut chunk_start_offset: usize = 12;
        let _package_count = arsc_table_header.package_count;
        let mut input: &[u8];
        while chunk_start_offset < self.binary_arsc.len() {
            input = &self.binary_arsc[chunk_start_offset..];

            let (_, chunk_header) =
                ChunkHeader::parse(input).map_err(|e| ParseError::ChunkHeader(e.to_string()))?;
            // println!("chunk header: {}", chunk_header);

            match chunk_header.typ {
                ChunkType::STRING_POOL => {
                    let string_chunk = &self.binary_arsc[chunk_start_offset..];
                    self.strings = parser::string_table(string_chunk)
                        .map_err(|e| ParseError::StringPool(e.to_string()))?;
                }
                ChunkType::TABLE_PACKAGE => {
                    let (_, package_chunk) = PackageChunkHeader::parse(input)
                        .map_err(|e| ParseError::PackageHeader(e.to_string()))?;
                    // println!("package chunk: {:?}", package_chunk);

                    //  The typeStrings field specifies the offset from the start of the Package chunk
                    let types_chunk = &self.binary_arsc
                        [chunk_start_offset + package_chunk.type_strings as usize..];
                    let (_, types_chunk_header) = ChunkHeader::parse(types_chunk)
                        .map_err(|e| ParseError::TypeStrings(e.to_string()))?;
                    let type_strings = parser::string_table(types_chunk)
                        .map_err(|e: ParseError| ParseError::TypeStrings(e.to_string()))?;

                    let key_chunk = &self.binary_arsc
                        [chunk_start_offset + package_chunk.key_strings as usize..];
                    let (_, key_chunk_header) = ChunkHeader::parse(key_chunk)
                        .map_err(|e| ParseError::KeyStrings(e.to_string()))?;
                    let key_strings = parser::string_table(key_chunk)
                        .map_err(|e: ParseError| ParseError::KeyStrings(e.to_string()))?;

                    let type_buffer_idx = chunk_start_offset
                        + types_chunk_header.chunk_size as usize
                        + key_chunk_header.chunk_size as usize
                        + package_chunk.header.header_size as usize;
                    let mut type_buffer = &self.binary_arsc[type_buffer_idx..];

                    // todo: move to while loop and check for end
                    let mut type_spec = Vec::new();
                    let mut types = Vec::new();
                    loop {
                        let (_, chunk_header) = ChunkHeader::parse(type_buffer)
                            .map_err(|e| ParseError::ChunkHeader(e.to_string()))?;
                        match chunk_header.typ {
                            ChunkType::TABLE_SPEC => {
                                let (_, type_spec_header) = TypeSpecChunkHeader::parse(type_buffer)
                                    .map_err(|e| ParseError::TypeSpecHeader(e.to_string()))?;
                                type_spec.push(type_spec_header);
                            }
                            ChunkType::TABLE_TYPE => {
                                /*
                                 * The TABLE_TYPE chunk is where the actual resource entries are stored.
                                 * Each entry corresponds to a specific resource in the application.
                                 * The code checks if the entry is a complex entry or a simple entry, and parses it accordingly.
                                 * Complex entries hold a set of name/value mappings, while simple entries hold a single value.
                                 * The parsed entries are stored in the ResEntry vector.
                                 */
                                let (type_buffer_next, type_chunk_header) =
                                    TypeChunkHeader::parse(type_buffer)
                                        .map_err(|e| ParseError::TypeChunkHeader(e.to_string()))?;
                                // The type identifier this chunk refers to.  Type IDs start at 1.
                                if type_chunk_header.id == 0 {
                                    // 0 is invalid
                                    println!("invalid type id: {}", type_chunk_header.id);
                                    continue;
                                }

                                let (_, entries) = parser::take_u32s(
                                    type_buffer_next,
                                    type_chunk_header.entry_count as usize,
                                )
                                .map_err(|e| ParseError::TypeChunkEntries(e.to_string()))?;

                                let map_buffer =
                                    &type_buffer[type_chunk_header.entries_start as usize..];
                                // println!("map buffer: {:?}", &map_buffer[..16]);

                                let mut res_entries = Vec::new();
                                for entry in entries {
                                    let mut current_entries = Vec::new();
                                    if entry == 0xFFFFFFFF {
                                        // no value for the resource, push empty entry
                                        res_entries.push(current_entries);
                                        continue;
                                    }

                                    let buffer = &map_buffer[entry as usize..];
                                    let (buffer_next, table_entry) =
                                        crate::nom_parser::TableEntry::parse(buffer)
                                            .map_err(|e| ParseError::TableEntry(e.to_string()))?;

                                    // TODO: other flags
                                    if table_entry.flags == TableEntryFlag::COMPLEX {
                                        // If set, this is a complex entry, holding a set of name/value
                                        // mappings.  It is followed by an array of ResTable_map structures.
                                        let (buffer_next, map_entry) =
                                            TableMapEntry::parse(buffer_next).map_err(|e| {
                                                ParseError::TableEntry(e.to_string())
                                            })?;

                                        let (_, entry_maps) =
                                            count(TableMap::parse, map_entry.count as usize)(
                                                buffer_next,
                                            )
                                            .map_err(|e| ParseError::TableEntry(e.to_string()))?;

                                        entry_maps.iter().for_each(|entry| {
                                            current_entries
                                                .push(entry.value.as_string(&self.strings));
                                        });
                                        res_entries.push(current_entries);
                                    } else {
                                        let mut current_entry = Vec::new();
                                        let (_, value_entry) = ResValue::parse(buffer_next)
                                            .map_err(|e| ParseError::TableEntry(e.to_string()))?;
                                        current_entry.push(value_entry.as_string(&self.strings));
                                        res_entries.push(current_entry);
                                    }
                                }
                                types.push((type_chunk_header.id.into(), res_entries));
                            }

                            // TODO: Do we need these?
                            ChunkType::TABLE_LIBRARY => {
                                unimplemented!(
                                    "Unimplemented table chunk type: {}",
                                    chunk_header.typ
                                );
                            }
                            ChunkType::TABLE_OVERLAYABLE => {
                                unimplemented!(
                                    "Unimplemented table chunk type: {}",
                                    chunk_header.typ
                                );
                            }
                            ChunkType::TABLE_STAGED_ALIAS => {
                                unimplemented!(
                                    "Unimplemented table chunk type: {}",
                                    chunk_header.typ
                                );
                            }
                            _ => {
                                println!("Unknown table chunk type: {}", chunk_header.typ);
                            }
                        }
                        if chunk_header.chunk_size as usize >= type_buffer.len() {
                            // println!("Reached end of type buffer");
                            break;
                        }
                        type_buffer = &type_buffer[chunk_header.chunk_size as usize..];
                    }

                    self.packages.insert(
                        package_chunk.id,
                        Package {
                            name: package_chunk.name,
                            type_strings,
                            key_strings,
                            type_spec,
                            types,
                        },
                    );
                }
                _ => {
                    println!("Unknown chunk type: {}", chunk_header.typ);
                    println!("Skipping chunk...");
                }
            }
            chunk_start_offset += chunk_header.chunk_size as usize;
        }

        Ok(self.binary_arsc.to_vec())
    }

    pub fn get_res_value(&self, res_id: u32) -> Option<String> {
        let package_id = res_id >> 24;
        let typ = (res_id >> 16) & 0xFF;
        let entry = res_id & 0xFFFF;

        let typ = self
            .packages
            .get(&package_id)
            .unwrap()
            .types
            .iter()
            .filter(|t| t.0 == typ)
            .collect::<Vec<_>>();
        let entry = typ
            .iter()
            .filter(|(_, v)| v.get(entry as usize).is_some())
            .filter_map(|(_, v)| v.get(entry as usize))
            .collect::<Vec<_>>();

        // Currently we return the first valid entry despite config settings
        let first_element = entry
            .iter()
            .filter_map(|inner_vec| inner_vec.iter().filter_map(|opt| opt.as_ref()).next())
            .next()
            .cloned();

        // if first_element == None {
        //     println!("entry: {:?}", entry);
        // }

        first_element
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};

    #[test]
    fn test_arsc_parser() -> Result<()> {
        let arsc_path = std::path::Path::new(
            "../data/arsc/3bb279f6dd8e9ef5dcc996733acda4b8ea3a184b5482f857bf0267adbe9c7d10.arsc",
        );
        let arsc_bytes: Vec<u8> = std::fs::read(arsc_path)?;
        let mut parser = Arsc::new(arsc_bytes.as_slice());
        let _arsc_bytes = parser
            .parse()
            .context(format!("Failed to parse arsc: {}", arsc_path.display()))?;
        assert!(parser
            .packages
            .get(&127u32)
            .unwrap()
            .type_strings
            .contains(&"attr".to_string()));
        Ok(())
    }

    #[test]
    fn test_resid_to_name() -> Result<()> {
        let res_id = 0x7f1101fd;

        let arsc_path = std::path::Path::new(
            "../data/arsc/3bb279f6dd8e9ef5dcc996733acda4b8ea3a184b5482f857bf0267adbe9c7d10.arsc",
        );
        let arsc_bytes: Vec<u8> = std::fs::read(arsc_path)?;
        let mut parser = Arsc::new(arsc_bytes.as_slice());
        let _arsc_bytes = parser
            .parse()
            .context(format!("Failed to parse arsc: {}", arsc_path.display()))?;

        assert_eq!(
            parser.get_res_value(res_id),
            Some("Frequently asked questions".to_string())
        );

        Ok(())
    }

    #[test]
    fn test_arsc_parser_all() -> Result<()> {
        let dir_path = std::path::Path::new("../data/arsc");
        let files = dir_path.read_dir()?;
        for file in files {
            let file = file?.path().canonicalize()?;
            if file.extension().map_or(false, |ext| ext != "arsc") {
                continue;
            }
            println!("\n---------- TESTING {:?} ----------", file);
            let file_bytes: Vec<u8> = std::fs::read(&file)?;
            let mut parser = Arsc::new(file_bytes.as_slice());
            parser
                .parse()
                .context(format!("Failed to parse arsc: {}", file.display()))?;
            println!("\n---------- EOF ----------");
        }

        Ok(())
    }
}
