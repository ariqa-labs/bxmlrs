use anyhow::Result;
use bxmlrs::parser;
use clap::Parser;
use path_clean::PathClean;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long = "file", value_parser)]
    file: Option<PathBuf>,

    #[clap(short, long = "dir", value_parser)]
    dir: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if let Some(dir_path) = args.dir {
        let dir = std::fs::read_dir(dir_path)?;
        for entry in dir {
            let entry = entry?;
            let file_path = entry.path().clean();
            print_manifest(&file_path)?;
        }
    } else if let Some(file_path) = args.file {
        print_manifest(&file_path)?;
    } else {
        println!("No file or directory specified.");
    }

    Ok(())
}

fn print_manifest(file_path: &Path) -> Result<()> {
    let mut parser = parser::Parser::from_file(file_path.to_str().unwrap())?;
    let manifest_bytes = parser.parse()?;

    let mut xml_reader = quick_xml::reader::Reader::from_str(std::str::from_utf8(&manifest_bytes)?);
    xml_reader.trim_text(true);

    println!("manifest:\n{}", std::str::from_utf8(&manifest_bytes)?);

    Ok(())
}
