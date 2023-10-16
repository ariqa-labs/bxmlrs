# BXMLRS

`bxmlrs` is a Rust library (WIP) for parsing binary Android XML files (`AndroidManifest.xml`).

## Usage

```rust
use bxmlrs::parser;
use quick_xml::reader::Reader;

let mut parser = parser::Parser::from_file(file_path.to_str().unwrap())?;
let manifest_bytes = parser.parse()?;

let mut xml_reader = Reader::from_str(std::str::from_utf8(&manifest_bytes)?);
xml_reader.trim_text(true);

println!("manifest:\n{}", std::str::from_utf8(&manifest_bytes)?);

```
