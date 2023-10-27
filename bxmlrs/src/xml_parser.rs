#![allow(dead_code)]

use nom::multi::count;
use nom::number::complete::{le_u16, le_u32};
use nom::{combinator::map, sequence::tuple, IResult};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Writer;
use std::io::Cursor;

use crate::arsc_parser::Arsc;
use crate::attributes;
use crate::nom_parser::{parser, ChunkHeader, ChunkType, ParseError, ResValue};

// Struct to represent parsed androidmanifest.xml file
#[derive(Clone, Debug)]
pub struct AndroidManifest<'bxml> {
  binary_xml: &'bxml [u8],
  strings: Vec<String>,
  resource_ids: Vec<u32>,
  xml_namespace: XmlNamespace,
}

impl<'bxml> AndroidManifest<'bxml> {
  pub fn new(binary_xml: &'bxml [u8]) -> Self {
    Self {
      binary_xml,
      strings: vec![],
      resource_ids: vec![],
      xml_namespace: XmlNamespace {
        prefix: "android".to_string(),
        uri: "http://schemas.android.com/apk/res/android".to_string(),
      },
    }
  }

  pub fn parse(&mut self, arsc: Option<&Arsc>) -> Result<Vec<u8>, ParseError> {
    let mut xml_writer = Writer::new(Cursor::new(Vec::new()));
    // <?xml version="1.0" encoding="utf-8"?>
    let decl = BytesDecl::from_start(BytesStart::from_content(
      "xml encoding='utf-8' version='1.1'",
      0,
    ));
    xml_writer
      .write_event(Event::Decl(decl))
      .map_err(|e| ParseError::BuildXml(e.to_string()))?;

    let (_, xml_chunk_header) =
      ChunkHeader::parse(self.binary_xml).map_err(|e| ParseError::ChunkHeader(e.to_string()))?;
    // println!(
    //     "xml size based on header: 0x{:x}",
    //     xml_chunk_header.chunk_size
    // );

    if xml_chunk_header.typ != ChunkType::XML {
      // Android doesn't seem to care about the xml type identifier
      println!(
        "[warning] Invalid xml chunk type: 0x{:x}",
        xml_chunk_header.typ
      );
    }
    // println!("xml chunk header: {}", xml_chunk_header);

    // todo: what if xml_chunk_header.headerSize specifies larger number than 8?
    // will string pool chunk follow?
    // - Seems like it content of xml_chunk_header can be anything: type_id, header_size, ?chunk_size? - nothing matters.

    let mut chunk_start_offset: usize = 8; // xml_chunk_header.header_size as usize;
    let mut input: &[u8];
    while chunk_start_offset < self.binary_xml.len() {
      input = &self.binary_xml[chunk_start_offset..];

      let (mut input, chunk_header) =
        ChunkHeader::parse(input).map_err(|e| ParseError::ChunkHeader(e.to_string()))?;
      // println!("chunk header: {}", chunk_header);

      // todo: what if strings_pool is not the first chunk?
      // to others will refer it
      match chunk_header.typ {
        ChunkType::STRING_POOL => {
          let string_chunk = &self.binary_xml[chunk_start_offset..];
          self.strings = parser::string_table(string_chunk)
            .map_err(|e: ParseError| ParseError::StringPool(e.to_string()))?;
        }
        ChunkType::XML_RESOURCE_MAP => {
          // RES_XML_LAST_CHUNK_TYPE           = 0x017f,
          // This contains a uint32_t array mapping strings in the string
          // pool back to resource identifiers.  It is optional.
          let elem_count =
            ((chunk_header.chunk_size - chunk_header.header_size as u32) / 4) as usize;
          let (_, resource_ids) =
            count(le_u32::<&[u8], nom::error::Error<&[u8]>>, elem_count)(input)
              .map_err(|e| ParseError::ResourceMap(e.to_string()))?;
          self.resource_ids = resource_ids;
        }

        ChunkType::XML_START_NAMESPACE => {
          input = &input[8..]; // skip lineNumber and comment fields

          let (_, ns) = self
            .parse_namespace(input)
            .map_err(|e| ParseError::StartNamespace(e.to_string()))?;
          self.xml_namespace = ns;
        }

        ChunkType::XML_START_ELEMENT => {
          input = &input[8..]; // skip lineNumber and comment fields

          let (_, xml_attr_ext) = self
            .parse_tree_attr_ext(input)
            .map_err(|e| ParseError::StartElement(e.to_string()))?;
          // println!("xml attr ext: {:?}", xml_attr_ext);
          let unknown = "UNKNOWN".to_string();
          let _elem_ns = self
            .strings
            .get(xml_attr_ext.ns as usize)
            .map_or(unknown.clone(), |s| s.clone());
          let elem_name = self
            .strings
            .get(xml_attr_ext.name as usize)
            .map_or(unknown, |s| s.clone());

          let mut xml_elem = BytesStart::new(elem_name);

          input = &input[xml_attr_ext.attribute_start as usize..];
          for _ in 0..xml_attr_ext.attribute_count {
            let (_, attr) =
              XMLTreeAttribute::parse(input).map_err(|e| ParseError::Attribute(e.to_string()))?;
            // TODO: what if attribute size is spoofed and set to a random number?
            // println!("attribute size: {:?}", xml_attr_ext.attribute_size);
            input = &input[xml_attr_ext.attribute_size as usize..];

            let _attr_ns = self
              .strings
              .get(attr.ns as usize)
              .unwrap_or(&"http://schemas.android.com/apk/res/android".to_owned())
              .clone();

            let attr_name: Option<String> = self
              .resource_ids
              .get(attr.name as usize)
              .and_then(|res_id| attributes::get_attribute_name(*res_id))
              .or_else(|| self.strings.get(attr.name as usize).cloned());

            // attribute value
            let mut attr_value: Option<String> = attr.typed_value.as_string(&self.strings);
            if let Some(arsc) = arsc {
              let mut rec_count = 0;
              while let Some(curr_attr_value) = &attr_value {
                if curr_attr_value.starts_with("@res/0x") && rec_count < 5 {
                  let res_id = u32::from_str_radix(&curr_attr_value[7..], 16).ok();
                  if let Some(res_id) = res_id {
                    let curr_arsc_value = arsc.get_res_value(res_id);
                    if let Some(curr_arsc_value) = curr_arsc_value {
                      // println!(
                      //     "old: {:?} curr_arsc_value: {:?} counter: {}",
                      //     attr_value, curr_arsc_value, rec_count
                      // );
                      attr_value = Some(curr_arsc_value);
                      rec_count += 1;
                      continue;
                    }
                  }
                }
                break;
              }
            }

            // println!("attribute: {:?}", attr);
            // println!("name: {:?}", attr_name);
            // println!("value: {:?}", attr_value);
            if let (Some(attr_name), Some(attr_value)) =
              (attr_name.as_deref(), attr_value.as_deref())
            {
              xml_elem.push_attribute((attr_name, attr_value));
            }
          }
          xml_writer
            .write_event(Event::Start(xml_elem.clone()))
            .map_err(|e| ParseError::StartElement(e.to_string()))?;
        }
        ChunkType::XML_END_ELEMENT => {
          input = &input[8..]; // skip lineNumber and comment fields

          // NOTE:
          // AndroidManifestNoNamespace.xml only contains ns and name at the end of the xml,
          // there is no data after that.
          let (_, xml_attr_ext) = self
            .parse_tree_attr_ext_end(input)
            .map_err(|e| ParseError::StartElement(e.to_string()))?;
          // println!("xml attr ext: {:?}", xml_attr_ext);
          let unknown = "UNKNOWN".to_string();
          let _elem_ns = self
            .strings
            .get(xml_attr_ext.ns as usize)
            .map_or(unknown.clone(), |s| s.clone());
          let elem_name = self
            .strings
            .get(xml_attr_ext.name as usize)
            .map_or(unknown, |s| s.clone());

          xml_writer
            .write_event(Event::End(BytesEnd::new(&elem_name)))
            .map_err(|e| ParseError::StartElement(e.to_string()))?;
        }
        // CDATA chunk
        // https://justanapplication.wordpress.com/2011/09/27/android-internals-binary-xml-part-eight-the-cdata-chunk
        ChunkType::XML_CDATA => { /* skip it */ }

        ChunkType::XML_END_NAMESPACE => {
          break;
        }
        _ => {
          println!("unknown chunk type: 0x{:x}", chunk_header.typ);
          break;
        }
      }
      chunk_start_offset += chunk_header.chunk_size as usize;
    }

    let xml_result = xml_writer.into_inner().into_inner();

    // // print as string
    // let result_str = std::str::from_utf8(&xml_result).unwrap();
    // println!("{}", result_str);

    Ok(xml_result)
  }

  fn parse_namespace<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], XmlNamespace> {
    map(tuple((le_u32, le_u32)), |(prefix, uri)| XmlNamespace {
      prefix: self.strings[prefix as usize].clone(),
      uri: self.strings[uri as usize].clone(),
    })(input)
  }

  fn parse_tree_attr_ext<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], XMLTreeAttrExt> {
    map(
      tuple((
        le_u32, le_u32, le_u16, le_u16, le_u16, le_u16, le_u16, le_u16,
      )),
      |(
        ns,
        name,
        attribute_start,
        attribute_size,
        attribute_count,
        id_index,
        class_index,
        style_index,
      )| XMLTreeAttrExt {
        ns,
        name,
        attribute_start,
        attribute_size,
        attribute_count,
        id_index,
        class_index,
        style_index,
      },
    )(input)
  }

  fn parse_tree_attr_ext_end<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], XMLTreeAttrExt> {
    map(tuple((le_u32, le_u32)), |(ns, name)| XMLTreeAttrExt {
      ns,
      name,
      attribute_start: 0,
      attribute_size: 0,
      attribute_count: 0,
      id_index: 0,
      class_index: 0,
      style_index: 0,
    })(input)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlNamespace {
  pub prefix: String,
  pub uri: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct XMLTreeAttribute {
  pub ns: u32,
  pub name: u32,
  // The original raw string value of this attribute.
  pub raw_value: u32,
  pub typed_value: ResValue,
}

impl XMLTreeAttribute {
  fn parse(input: &[u8]) -> IResult<&[u8], XMLTreeAttribute> {
    map(
      tuple((le_u32, le_u32, le_u32, ResValue::parse)),
      |(ns, name, raw_value, typed_value)| XMLTreeAttribute {
        ns,
        name,
        raw_value,
        typed_value,
      },
    )(input)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XMLTreeAttrExt {
  // String of the full namespace of this element.
  pub ns: u32,
  // String name of this node if it is an ELEMENT; the raw
  // character data if this is a CDATA node.
  pub name: u32,
  // Byte offset from the start of this structure where the attributes start.
  pub attribute_start: u16,
  // Size of the XMLTree_attribute structures that follow.
  pub attribute_size: u16,
  // Number of attributes associated with an ELEMENT.  These are
  // available as an array of ResXMLTreeAttribute structures
  // immediately following this node.
  pub attribute_count: u16,
  // Index (1-based) of the "id" attribute. 0 if none.
  pub id_index: u16,
  // Index (1-based) of the "class" attribute. 0 if none.
  pub class_index: u16,
  // Index (1-based) of the "style" attribute. 0 if none.
  pub style_index: u16,
}

pub struct ResourceMapChunk {}

#[cfg(test)]
mod tests {
  use super::*;
  use anyhow::{Context, Result};

  #[test]
  fn test_xml_parser() -> Result<()> {
    let manifest_path =
      std::path::Path::new("../data/xml/AndroidManifest-com.simplemobiletools.gallery.pro.xml");
    let manifest_bytes: Vec<u8> = std::fs::read(manifest_path)?;
    let mut parser = AndroidManifest::new(manifest_bytes.as_slice());
    parser.parse(None).context(format!(
      "Failed to parse manifest: {}",
      manifest_path.display()
    ))?;
    assert!(parser.strings.contains(&"theme".to_string()));

    // println!("--- strings ---");
    // for (i, s) in parser.strings.iter().enumerate() {
    //     println!("{}: {}", i, s);
    // }

    Ok(())
  }

  #[test]
  fn test_xml_parser_all() -> Result<()> {
    let dir_path = std::path::Path::new("../data/xml");
    let files = dir_path.read_dir()?;
    for file in files {
      let file = file?.path().canonicalize()?;
      if file.extension().map_or(false, |ext| ext != "xml") {
        continue;
      }
      println!("\n---------- TESTING {:?} ----------", file);
      let file_bytes: Vec<u8> = std::fs::read(&file)?;
      let mut parser = AndroidManifest::new(file_bytes.as_slice());
      parser
        .parse(None)
        .context(format!("Failed to parse manifest: {}", file.display()))?;
      println!("\n---------- EOF ----------");
    }

    Ok(())
  }
}
