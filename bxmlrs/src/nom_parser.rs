#![allow(dead_code)]

use nom::multi::count;
use nom::number::complete::{le_u16, le_u32, le_u8};
use nom::{combinator::map, sequence::tuple, IResult};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
  #[error("Parsing error: {0}")]
  Generic(String),

  #[error("Failed to parse chunk header: {0}")]
  ChunkHeader(String),

  #[error("Failed to parse string pool header: {0}")]
  StringPoolHeader(String),

  #[error("Buffer not enough: {0}")]
  BufferNotEnough(String),

  #[error("Failed to parse string pool: {0}")]
  StringPool(String),

  #[error("Failed to parse string within string pool: {0}")]
  String(String),

  #[error("Failed to parse resource map: {0}")]
  ResourceMap(String),

  #[error("Failed to parse start namespace: {0}")]
  StartNamespace(String),

  #[error("Failed to parse end namespace: {0}")]
  EndNamespace(String),

  #[error("Failed to parse start element: {0}")]
  StartElement(String),

  #[error("Failed to parse attribute: {0}")]
  Attribute(String),

  #[error("Failed to build XML tree: {0}")]
  BuildXml(String),

  #[error("Failed to parse package header: {0}")]
  PackageHeader(String),

  #[error("Failed to parse type strings: {0}")]
  TypeStrings(String),

  #[error("Failed to parse key strings: {0}")]
  KeyStrings(String),

  #[error("Failed to parse type spec header: {0}")]
  TypeSpecHeader(String),

  #[error("Failed to parse type chunk header: {0}")]
  TypeChunkHeader(String),

  #[error("Failed to parse type chunk entries: {0}")]
  TypeChunkEntries(String),

  #[error("Failed to parse table entry: {0}")]
  TableEntry(String),

  #[error("Failed to parse zip file: {0}")]
  Zip(String),

  #[error("Failed to open file: {0}")]
  File(String),
  // TODO: Add more error variants as needed
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkHeader {
  // Type identifier for this chunk.  The meaning of this value depends
  // on the containing chunk.
  pub typ: u16,
  // Size of the chunk header (in bytes).  Adding this value to
  // the address of the chunk allows you to find its associated data
  // (if any).
  pub header_size: u16,

  // Total size of this chunk (in bytes).  This is the chunkSize plus
  // the size of any data associated with the chunk.  Adding this value
  // to the chunk allows you to completely skip its contents (including
  // any child chunks).  If this value is the same as chunkSize, there is
  // no data associated with the chunk.
  pub chunk_size: u32,
}

impl ChunkHeader {
  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], ChunkHeader> {
    map(
      tuple((le_u16, le_u16, le_u32)),
      |(typ, header_size, chunk_size)| ChunkHeader {
        typ,
        header_size,
        chunk_size,
      },
    )(input)
  }
}

impl std::fmt::Display for ChunkHeader {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    write!(
      f,
      "ChunkHeader {{ typ: 0x{:x}, header_size: 0x{:x}, chunk_size: 0x{:x} }}",
      self.typ, self.header_size, self.chunk_size
    )
  }
}

pub struct ChunkType;

impl ChunkType {
  pub const NULL: u16 = 0x0000;
  pub const STRING_POOL: u16 = 0x0001;
  pub const TABLE: u16 = 0x0002;
  pub const XML: u16 = 0x0003;

  pub const XML_START_NAMESPACE: u16 = 0x0100;
  pub const XML_END_NAMESPACE: u16 = 0x0101;
  pub const XML_START_ELEMENT: u16 = 0x0102;
  pub const XML_END_ELEMENT: u16 = 0x0103;
  pub const XML_CDATA: u16 = 0x0104;
  pub const XML_LAST_CHUNK: u16 = 0x017f;
  pub const XML_RESOURCE_MAP: u16 = 0x0180;

  pub const TABLE_PACKAGE: u16 = 0x0200;
  pub const TABLE_TYPE: u16 = 0x0201;
  pub const TABLE_SPEC: u16 = 0x0202;
  pub const TABLE_LIBRARY: u16 = 0x0203;
  pub const TABLE_OVERLAYABLE: u16 = 0x0204;
  pub const TABLE_OVERLAYABLE_POLICY: u16 = 0x0205;
  pub const TABLE_STAGED_ALIAS: u16 = 0x0206;
}

/**
 * Definition for a pool of strings.  The data of this chunk is an
 * array of uint32_t providing indices into the pool, relative to
 * stringsStart.  At stringsStart are all of the PackageHeader
 * If styleCount is not zero, then immediately following the array of
 * uint32_t indices into the string table is another array of indices
 * into a style table starting at stylesStart.  Each entry in the
 * style table is an array of ResStringPool_span structures.
 */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringPoolChunk {
  pub header: ChunkHeader,
  pub string_count: u32,
  pub style_count: u32,
  pub flags: u32,
  pub is_utf8: bool,
  pub strings_start: u32,
  pub styles_start: u32,
}

impl StringPoolChunk {
  fn extract_string(
    &self,
    string_buffer: &[u8],
  ) -> Result<String, ParseError> {
    if self.is_utf8 {
      let (mut string_buffer, str_len) = le_u8::<_, nom::error::Error<&[u8]>>(string_buffer)
        .map_err(|e| ParseError::String(e.to_string()))?;
      string_buffer = &string_buffer[1..];
      if string_buffer.len() < str_len as usize {
        return Err(ParseError::BufferNotEnough("Not enough bytes".to_string()));
      }
      let (_, str_bytes) =
        count(le_u8::<_, nom::error::Error<&[u8]>>, str_len as usize)(string_buffer)
          .map_err(|e| ParseError::String(e.to_string()))?;
      let null_pos = str_bytes
        .iter()
        .position(|&x| x == 0)
        .unwrap_or(str_bytes.len());
      let str_bytes = &str_bytes[..null_pos];
      Ok(String::from_utf8_lossy(str_bytes).to_string())
    } else {
      let (string_buffer, str_len) = le_u16::<_, nom::error::Error<&[u8]>>(string_buffer)
        .map_err(|e| ParseError::String(e.to_string()))?;
      if string_buffer.len() < str_len as usize * 2 {
        return Err(ParseError::BufferNotEnough("Not enough bytes".to_string()));
      }
      let (_, str_bytes) =
        count(le_u16::<_, nom::error::Error<&[u8]>>, str_len as usize)(string_buffer)
          .map_err(|e| ParseError::String(e.to_string()))?;
      let null_pos = str_bytes
        .iter()
        .position(|&x| x == 0)
        .unwrap_or(str_bytes.len());
      let str_bytes = &str_bytes[..null_pos];
      Ok(String::from_utf16_lossy(str_bytes))
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableHeader {
  pub header: ChunkHeader,
  // specifies the number of packages contained in this table
  pub package_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageChunkHeader {
  pub header: ChunkHeader,
  // The id field specifies the numeric id of the Package.
  // It is used as part of the identity of each Resource defined in the Package.
  pub id: u32,
  // The name field specifies the symbolic name of the Package.
  pub name: String, // [u16; 128],
  // The type_strings field specifies the offset from the start of the Package chunk to the start of the typeStrings StringPool chunk.
  pub type_strings: u32,
  pub last_public_type: u32,
  pub key_strings: u32,
  pub last_public_key: u32,
  // pub type_id_offset: u32, // was added later, may not present
}

impl PackageChunkHeader {
  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], PackageChunkHeader> {
    let (
      input,
      (header, id, name_bytes, type_strings, last_public_type, key_strings, last_public_key),
    ) = tuple((
      ChunkHeader::parse,
      le_u32,
      count(le_u16, 128),
      le_u32,
      le_u32,
      le_u32,
      le_u32,
    ))(input)?;

    let null_pos = name_bytes
      .iter()
      .position(|&x| x == 0)
      .unwrap_or(name_bytes.len());
    let name_bytes = &name_bytes[..null_pos];
    let name = String::from_utf16_lossy(name_bytes);

    let header = PackageChunkHeader {
      header,
      id,
      name,
      type_strings,
      last_public_type,
      key_strings,
      last_public_key,
    };

    Ok((input, header))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeSpecChunkHeader {
  pub header: ChunkHeader,
  // The type identifier this chunk is holding.  Type IDs start
  // at 1 (corresponding to the value of the type bits in a
  // resource identifier).  0 is invalid.
  // It is the string at index id - 1 in the typeStrings StringPool chunk in the containing Package chunk
  pub type_id: u8,
  res0: u8,
  res1: u16,

  // Number of uint32_t entry configuration masks that follow.
  // The entryCount field specifies the number of entries in the body of this chunk.
  pub entry_count: u32,
  // pub flag: u32,
}

impl TypeSpecChunkHeader {
  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], (TypeSpecChunkHeader, Vec<u32>)> {
    let (input, (header, type_id, res0, res1, entry_count)) =
      tuple((ChunkHeader::parse, le_u8, le_u8, le_u16, le_u32))(input)?;

    let (input, entry_flags) = count(le_u32, entry_count as usize)(input)?;

    Ok((
      input,
      (
        TypeSpecChunkHeader {
          header,
          type_id,
          res0,
          res1,
          entry_count,
        },
        entry_flags,
      ),
    ))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeChunkConfig {
  pub structure_size: u32,
  // TODO: add later if necessary
  pub data: Vec<u8>,
}
struct TypeChunkFlags;
impl TypeChunkFlags {
  // If set, the entry is sparse, and encodes both the entry ID and offset into each entry,
  // and a binary search is used to find the key. Only available on platforms >= O.
  // Mark any types that use this with a v26 qualifier to prevent runtime issues on older
  // platforms.
  pub const SPARSE: u8 = 0x01;
  // If set, the offsets to the entries are encoded in 16-bit, real_offset = offset * 4u
  // An 16-bit offset of 0xffffu means a NO_ENTRY
  pub const OFFSET16: u8 = 0x02;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeChunkHeader {
  pub header: ChunkHeader,
  pub id: u8,
  pub flags: u8,
  pub res1: u16,
  pub entry_count: u32,
  // The entries_start field specifies the offset from the start of the chunk to the start of the entries in the body of the chunk which represent the Resource values.
  pub entries_start: u32,
  pub config: TypeChunkConfig,
}

impl TypeChunkHeader {
  fn type_chunk_config(input: &[u8]) -> IResult<&[u8], TypeChunkConfig> {
    let (input, total_size) = le_u32(input)?;

    let (input, data) = count(le_u8, total_size as usize - 4)(input)?;
    Ok((
      input,
      TypeChunkConfig {
        structure_size: total_size,
        data,
      },
    ))
  }

  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], TypeChunkHeader> {
    let (input, (header, id, flags, res1, entry_count, entries_start, config)) = tuple((
      ChunkHeader::parse,
      le_u8,
      le_u8,
      le_u16,
      le_u32,
      le_u32,
      Self::type_chunk_config,
    ))(input)?;

    Ok((
      input,
      TypeChunkHeader {
        header,
        id,
        flags,
        res1,
        entry_count,
        entries_start,
        config,
      },
    ))
  }
}

pub(crate) struct TableEntryFlag;
impl TableEntryFlag {
  // If set, this is a complex entry, holding a set of name/value
  // mappings.  It is followed by an array of ResTable_map structures.
  pub const COMPLEX: u16 = 0x0001;
  // If set, this resource has been declared public, so libraries
  // are allowed to reference it.
  pub const PUBLIC: u16 = 0x0002;
  // If set, this is a weak resource and may be overriden by strong
  // resources of the same name/type. This is only useful during
  // linking with other resource tables.
  pub const WEAK: u16 = 0x0004;
  // If set, this is a compact entry with data type and value directly
  // encoded in the this entry, see ResTable_entry::compact
  pub const COMPACT: u16 = 0x0008;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TableEntry {
  pub size: u16,
  pub flags: u16,
  pub string_index: u32,
}

impl TableEntry {
  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], TableEntry> {
    let (input, size) = le_u16(input)?;
    let (input, flags) = le_u16(input)?;
    let (input, string_index) = le_u32(input)?;
    Ok((
      input,
      TableEntry {
        size,
        flags,
        string_index,
      },
    ))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TableMapEntry {
  // Resource identifier of the parent mapping, or 0 if there is none.
  // This is always treated as a TYPE_DYNAMIC_REFERENCE.
  pub parent: u32,
  // Number of name/value pairs that follow for FLAG_COMPLEX.
  pub count: u32,
}

impl TableMapEntry {
  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], TableMapEntry> {
    let (input, parent) = le_u32(input)?;
    let (input, count) = le_u32(input)?;
    Ok((input, TableMapEntry { parent, count }))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TableMap {
  // The resource identifier defining this mapping's name.  For attribute
  // resources, 'name' can be one of the following special resource types
  // to supply meta-data about the attribute; for all other resource types
  // it must be an attribute resource.
  pub name: u32,

  pub value: ResValue,
}

impl TableMap {
  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], TableMap> {
    let (input, name) = le_u32(input)?;
    let (input, value) = ResValue::parse(input)?;
    Ok((input, TableMap { name, value }))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResValue {
  // Number of bytes in this structure.
  pub size: u16,
  // Always set to 0.
  pub res0: u8,
  // Type of the data value.
  pub data_type: u8, /* ResType */
  // The data for this item, as interpreted according to dataType.
  pub data: u32,
}

impl ResValue {
  pub(crate) fn parse(input: &[u8]) -> IResult<&[u8], ResValue> {
    map(
      tuple((le_u16, le_u8, le_u8, le_u32)),
      |(size, _, data_type, data)| ResValue {
        size,
        res0: 0,
        data_type,
        data,
      },
    )(input)
  }

  pub(crate) fn as_string(
    &self,
    strings: &[String],
  ) -> Option<String> {
    match self.data_type {
      ResType::STRING => strings.get(self.data as usize).cloned(),
      ResType::INT_BOOLEAN => Some((if self.data != 0 { "true" } else { "false" }).to_string()),
      ResType::INT_DEC => Some(format!("{}", self.data)),
      ResType::INT_HEX => Some(format!("0x{:X}", self.data)),
      ResType::FLOAT => Some(format!("{:.2}", self.data as f32)),
      ResType::REFERENCE => Some(format!("@res/0x{:x}", self.data)),
      ResType::DYNAMIC_REFERENCE => Some(format!("@dyn/0x{:X}", self.data)),
      ResType::ATTRIBUTE => Some(format!("@attr/0x{:x}", self.data)),
      _ => None,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResType;
impl ResType {
  // Contains no data.
  pub const NULL: u8 = 0x00;
  // The 'data' holds a ResTable_ref, a reference to another resource
  // table entry.
  pub const REFERENCE: u8 = 0x01;
  // The 'data' holds an attribute resource identifier.
  pub const ATTRIBUTE: u8 = 0x02;
  // The 'data' holds an index into the containing resource table's
  // global value string pool.
  pub const STRING: u8 = 0x03;
  // The 'data' holds a single-precision floating point number.
  pub const FLOAT: u8 = 0x04;
  // The 'data' holds a complex number encoding a dimension value,
  // such as "100in".
  pub const DIMENSION: u8 = 0x05;
  // The 'data' holds a complex number encoding a fraction of a
  // container.
  pub const FRACTION: u8 = 0x06;

  pub const DYNAMIC_REFERENCE: u8 = 0x07;
  pub const INT_DEC: u8 = 0x10;
  pub const INT_HEX: u8 = 0x11;
  pub const INT_BOOLEAN: u8 = 0x12;
}

pub mod parser {
  use super::*;

  pub(crate) fn take_u32s(
    buffer: &[u8],
    len: usize,
  ) -> IResult<&[u8], Vec<u32>> {
    count(le_u32::<_, nom::error::Error<&[u8]>>, len)(buffer)
  }

  // section blob - is a buffer which contains data referenced by string_pool_chunk.strings_start
  // string_pool_chunk - start of sting pool chunk
  pub(crate) fn string_table(string_chunk: &[u8]) -> Result<Vec<String>, ParseError> {
    let (string_chunk_next, string_pool_chunk) = parse_string_pool_header(string_chunk)
      .map_err(|e| ParseError::StringPoolHeader(e.to_string()))?;
    // println!("string pool chunk: {:?}", string_pool_chunk);

    // we need to get string_count size of u32 from the chunk body
    let (_string_chunk_next, string_offsets) = count(
      le_u32::<&[u8], nom::error::Error<&[u8]>>,
      string_pool_chunk.string_count as usize,
    )(string_chunk_next)
    .map_err(|e| ParseError::StringPool(e.to_string()))?;
    // println!("string offsets: {:?}", string_offsets);

    // todo: does string start offset matter? yes
    // If styleCount is not zero, then immediately following the array of
    // uint32_t indices into the string table is another array of indices
    // into a style table starting at stylesStart.  Each entry in the
    // style table is an array of ResStringPool_span structures.
    // string_start + 8?

    let strings_start = &string_chunk[string_pool_chunk.strings_start as usize..];
    let strings = read_strings(strings_start, &string_pool_chunk, string_offsets)?;

    // Each entry in the style table is an array of ResStringPool_span structures.
    if string_pool_chunk.style_count > 0 {
      // let (_, style_offsets) = count(
      //     le_u32::<&[u8], nom::error::Error<&[u8]>>,
      //     string_pool_chunk.style_count as usize,
      // )(string_chunk_next)
      // .map_err(|e: nom::Err<nom::error::Error<&[u8]>>| {
      //     ParseError::StringPool(e.to_string())
      // })?;
    }

    Ok(strings)
  }

  fn read_strings(
    strings_buffer: &[u8],
    string_pool_chunk: &StringPoolChunk,
    string_offsets: Vec<u32>,
  ) -> Result<Vec<String>, ParseError> {
    // NOTE: We need to count even empty strings because they are indexed by id
    // println!("index: {} - string: {}", self.strings.len(), curr_string);
    let mut strings = vec![];

    for offset in string_offsets {
      if offset >= strings_buffer.len() as u32 {
        break;
      }
      let string_buffer: &[u8] = &strings_buffer[offset as usize..];
      match string_pool_chunk.extract_string(string_buffer) {
        Ok(string) => strings.push(string),
        Err(err) => match err {
          // Sometimes the buffer is not enough even though there are more string offsets
          // Usually that's due to obfuscation techniques
          ParseError::BufferNotEnough(_) => {
            break;
          }
          _ => return Err(err),
        },
      }
    }
    Ok(strings)
  }

  pub(crate) fn parse_string_pool_header(input: &[u8]) -> IResult<&[u8], StringPoolChunk> {
    map(
      tuple((ChunkHeader::parse, le_u32, le_u32, le_u32, le_u32, le_u32)),
      |(header, string_count, style_count, flags, strings_start, styles_start)| StringPoolChunk {
        header,
        string_count,
        style_count,
        flags,
        is_utf8: (flags & 0x100) > 0,
        strings_start,
        styles_start,
      },
    )(input)
  }

  pub(crate) fn parse_table(input: &[u8]) -> IResult<&[u8], TableHeader> {
    map(
      tuple((ChunkHeader::parse, le_u32)),
      |(header, package_count)| TableHeader {
        header,
        package_count,
      },
    )(input)
  }
}
