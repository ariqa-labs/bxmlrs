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

let mut package_name = String::new();
loop {
        match xml_reader.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(event) => match event {
                quick_xml::events::Event::Start(e) => match e.name() {
                    QName(b"manifest") => {
                        if let Some(attr) = e.try_get_attribute("package")? {
                            package_name = attr.unescape_value().unwrap_or_default().into_owned();
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            Err(e) => {}
        }
    }
```

Check `bxmlrs-bin` for a full example.

