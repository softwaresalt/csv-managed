use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use serde_yaml::Value as YamlValue;

pub trait YamlProvider: Send + Sync {
    fn parse_str(&self, input: &str) -> Result<YamlValue>;
    fn dump_value(&self, value: &YamlValue) -> Result<String>;
}

struct SerdeYamlProvider;

impl YamlProvider for SerdeYamlProvider {
    fn parse_str(&self, input: &str) -> Result<YamlValue> {
        Ok(serde_yaml::from_str(input)?)
    }

    fn dump_value(&self, value: &YamlValue) -> Result<String> {
        Ok(serde_yaml::to_string(value)?)
    }
}

static YAML_PROVIDER: OnceLock<Box<dyn YamlProvider>> = OnceLock::new();

pub fn provider() -> &'static dyn YamlProvider {
    YAML_PROVIDER
        .get_or_init(|| Box::new(SerdeYamlProvider))
        .as_ref()
}

/// Install a custom YAML provider. Intended for future experimentation and tests.
pub fn set_provider(provider: Box<dyn YamlProvider>) -> std::result::Result<(), &'static str> {
    YAML_PROVIDER
        .set(provider)
        .map_err(|_| "YAML provider already set")
}

fn read_to_string(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("Opening YAML file {path:?}"))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

fn write_string(path: &Path, contents: &str) -> Result<()> {
    let mut file = File::create(path).with_context(|| format!("Creating YAML file {path:?}"))?;
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub fn load_from_path<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = read_to_string(path)?;
    let value = provider().parse_str(&raw)?;
    Ok(serde_yaml::from_value(value)?)
}

pub fn save_to_path<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    let value = serde_yaml::to_value(data)?;
    let serialized = provider().dump_value(&value)?;
    write_string(path, &serialized)
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    let yaml_value = serde_yaml::to_value(value)?;
    provider().dump_value(&yaml_value)
}

pub fn to_value<T: Serialize>(value: &T) -> Result<YamlValue> {
    Ok(serde_yaml::to_value(value)?)
}
