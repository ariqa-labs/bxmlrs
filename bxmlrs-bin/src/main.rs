use anyhow::Result;
use bxmlrs::parser;
use clap::Parser;
use path_clean::PathClean;
use quick_xml::name::QName;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct IntentFilter {
    pub action: String,
    pub categories: Vec<String>,
}

#[derive(Debug)]
pub struct Component {
    pub name: String,
    pub intent_filters: Vec<IntentFilter>,
}

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
    let mut parser = parser::Parser::from_file(file_path)?;
    let manifest_bytes = parser.parse()?;

    let mut xml_reader = quick_xml::reader::Reader::from_str(std::str::from_utf8(&manifest_bytes)?);
    xml_reader.trim_text(true);

    let mut activities: Vec<Component> = Vec::new();
    let mut receivers: Vec<Component> = Vec::new();
    let mut services: Vec<Component> = Vec::new();
    let mut providers: Vec<String> = Vec::new();
    let mut stack: Vec<Component> = Vec::new();

    let mut permissions: Vec<String> = Vec::new();

    let mut min_sdk = String::new();
    let mut target_sdk = String::new();

    let mut application_class = String::new();
    let mut application_name = String::new();
    let mut package_name = String::new();
    let mut icon = String::new();

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
                    QName(b"application") => {
                        if let Some(attr) = e.try_get_attribute("label")? {
                            application_name =
                                attr.unescape_value().unwrap_or_default().into_owned();
                        }
                        if let Some(attr) = e.try_get_attribute("name")? {
                            application_class =
                                attr.unescape_value().unwrap_or_default().into_owned();
                        }
                        if let Some(attr) = e.try_get_attribute("icon")? {
                            icon = attr.unescape_value().unwrap_or_default().into_owned();
                        }
                    }
                    QName(b"activity") | QName(b"receiver") | QName(b"service") => {
                        let activity = Component {
                            name: e
                                .try_get_attribute("name")?
                                .map(|attr| attr.unescape_value().unwrap_or_default().into_owned())
                                .unwrap_or_default(),
                            intent_filters: Vec::new(),
                        };
                        stack.push(activity);
                    }
                    QName(b"provider") => {
                        if let Some(attr) = e.try_get_attribute("name")? {
                            providers.push(attr.unescape_value().unwrap_or_default().into_owned())
                        }
                    }
                    QName(b"intent-filter") => {
                        let intent_filter = IntentFilter {
                            action: String::new(),
                            categories: Vec::new(),
                        };
                        if let Some(activity) = stack.last_mut() {
                            activity.intent_filters.push(intent_filter);
                        }
                    }
                    QName(b"uses-sdk") => {
                        if let Some(attr) = e.try_get_attribute("minSdkVersion")? {
                            min_sdk = attr.unescape_value().unwrap_or_default().into_owned();
                        }
                        if let Some(attr) = e.try_get_attribute("targetSdkVersion")? {
                            target_sdk = attr.unescape_value().unwrap_or_default().into_owned();
                        }
                    }
                    QName(b"uses-permission")
                    | QName(b"permission")
                    | QName(b"permission-tree")
                    | QName(b"permission-group") => {
                        if let Some(attr) = e.try_get_attribute("name")? {
                            permissions
                                .push(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                    }
                    QName(b"action") => {
                        if let Some(intent_filter) = stack
                            .last_mut()
                            .and_then(|component| component.intent_filters.last_mut())
                        {
                            if let Some(attr) = e.try_get_attribute("name")? {
                                intent_filter.action =
                                    attr.unescape_value().unwrap_or_default().into_owned();
                            }
                        }
                    }
                    QName(b"category") => {
                        if let Some(intent_filter) = stack
                            .last_mut()
                            .and_then(|component| component.intent_filters.last_mut())
                        {
                            if let Some(attr) = e.try_get_attribute("name")? {
                                intent_filter
                                    .categories
                                    .push(attr.unescape_value().unwrap_or_default().into_owned());
                            }
                        }
                    }
                    _ => {}
                },
                quick_xml::events::Event::End(e) => match e.name() {
                    QName(b"activity") => {
                        if let Some(activity) = stack.pop() {
                            activities.push(activity);
                        }
                    }
                    QName(b"receiver") => {
                        if let Some(activity) = stack.pop() {
                            receivers.push(activity);
                        }
                    }
                    QName(b"service") => {
                        if let Some(activity) = stack.pop() {
                            services.push(activity);
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            Err(e) => {
                println!(
                    "Error at position {}: {:?}",
                    xml_reader.buffer_position(),
                    e
                );
            }
        }
    }

    println!("activities");
    for activity in activities {
        println!("{:?}", activity);
    }
    println!("receivers");
    for activity in receivers {
        println!("{:?}", activity);
    }
    println!("services");
    for activity in services {
        println!("{:?}", activity);
    }
    println!("providers");
    for provider in providers {
        println!("{:?}", provider);
    }

    println!("min_sdk: {:?}", min_sdk);
    println!("target_sdk: {:?}", target_sdk);

    println!("package_name: {:?}", package_name);
    println!("application_name: {:?}", application_name);
    println!("application_class: {:?}", application_class);
    println!("icon: {:?}", icon);

    println!("permissions");
    for permission in permissions {
        println!("{:?}", permission);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print_manifest;

    #[test]
    fn test_apks() -> Result<()> {
        let dir_path = std::path::Path::new("../data/apk");
        let files = dir_path.read_dir()?;
        for file in files {
            let file = file?.path().canonicalize()?;
            println!("\n---------- TESTING {:?} ----------", file);
            print_manifest(&file)?;
            println!("---------- END ----------");
        }

        Ok(())
    }
}
