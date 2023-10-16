use crate::arsc_parser::Arsc;
use crate::nom_parser::ParseError;
use crate::xml_parser::AndroidManifest;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct Parser {
    arsc_raw: Vec<u8>,
    manifest_raw: Vec<u8>,
}

impl Parser {
    pub fn from_file(file_path: &Path) -> Result<Self, ParseError> {
        let file = File::open(file_path).map_err(|e| ParseError::File(e.to_string()))?;
        let mut archive = ZipArchive::new(file).map_err(|e| ParseError::Zip(e.to_string()))?;
        let mut manifest_raw: Vec<u8> = Vec::new();
        let mut arsc_raw: Vec<u8> = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| ParseError::Zip(e.to_string()))?;

            if file.name() == "AndroidManifest.xml" {
                let mut data = Vec::new();
                file.read_to_end(&mut data)
                    .map_err(|e| ParseError::Zip(e.to_string()))?;
                manifest_raw = data;
            } else if file.name() == "resources.arsc" {
                let mut data = Vec::new();
                file.read_to_end(&mut data)
                    .map_err(|e| ParseError::Zip(e.to_string()))?;
                arsc_raw = data;
            }
        }

        Ok(Self {
            arsc_raw: arsc_raw.clone(),
            manifest_raw: manifest_raw.clone(),
        })
    }

    pub fn parse(&mut self) -> Result<Vec<u8>, ParseError> {
        let mut arsc_parser = Arsc::new(&self.arsc_raw);
        arsc_parser.parse()?;

        let mut manifest_parser = AndroidManifest::new(&self.manifest_raw);
        let parsed_manifest_bytes = manifest_parser.parse(Some(&arsc_parser))?;

        Ok(parsed_manifest_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use quick_xml::reader::Reader;

    #[test]
    fn test_parser() -> Result<()> {
        let parsed_manifest_bytes =
            Parser::from_file("../data/apk/com.etb.filemanager_3.apk".as_ref())?.parse()?;
        let mut reader = Reader::from_str(std::str::from_utf8(&parsed_manifest_bytes)?);
        reader.trim_text(true);

        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (),
            }
        }

        println!("{}", std::str::from_utf8(&parsed_manifest_bytes)?);
        Ok(())
    }
}
