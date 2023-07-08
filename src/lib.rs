use std::{collections::HashMap, error::Error, fs, path::PathBuf};

use hcl::ObjectKey;
use thiserror::Error;

#[derive(Debug)]
pub struct Module {
    pub path: PathBuf,
    pub required_core: Vec<String>,
    pub required_providers: HashMap<String, ProviderRequirement>,
}

impl Module {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            required_core: vec![],
            required_providers: HashMap::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ProviderRequirement {
    pub source: String,
    pub version_constraints: Vec<String>,
    pub configuration_aliases: Vec<ProviderRef>,
}

impl ProviderRequirement {
    pub fn new(source: String, version_constraints: Vec<String>) -> Self {
        Self {
            source,
            version_constraints,
            configuration_aliases: vec![],
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProviderRef {
    pub name: String,
    pub alias: String,
}

impl ProviderRef {
    pub fn new(name: String, alias: String) -> Self {
        Self { name, alias }
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum ParseError {
    #[error(r#"Found multiple source attributes for provider {name:?}: "{provider_source:?}", "{duplicate_source:?}""#)]
    MultipleSourcesForProvider {
        name: String,
        provider_source: String,
        duplicate_source: String,
    },
}

/// Reads the directory at the given path and attempts to interpret it as a Terraform module.
pub fn load_module(path: &PathBuf) -> Result<Module, Box<dyn Error>> {
    let mut module = Module::new(path.clone());

    let files = get_files_in_dir(path)?;

    for file_name in files {
        let file_contents = fs::read_to_string(&file_name)?;
        let file = hcl::parse(&file_contents)?;

        load_module_from_file(file, &mut module)?;
    }

    Ok(module)
}

/// Reads given file, interprets it and stores in given [`Module`][Module]
pub fn load_module_from_file(file: hcl::Body, module: &mut Module) -> Result<(), ParseError> {
    for block in file.blocks() {
        let body = block.body();

        #[allow(clippy::all)]
        match block.identifier() {
            "terraform" => handle_terraform_block(body, module)?,
            _ => (),
        }
    }

    Ok(())
}

fn handle_terraform_block(body: &hcl::Body, module: &mut Module) -> Result<(), ParseError> {
    body.attributes()
        .filter(|attr| attr.key() == "required_version")
        .for_each(|attr| {
            module
                .required_core
                .push(attr.expr().to_string().replace('"', ""))
        });

    for inner_block in body.blocks() {
        #[allow(clippy::all)]
        match inner_block.identifier() {
            "required_providers" => handle_required_providers_block(inner_block.body(), module)?,
            _ => (),
        }
    }

    Ok(())
}

fn handle_required_providers_block(
    body: &hcl::Body,
    module: &mut Module,
) -> Result<(), ParseError> {
    for provider in body.attributes() {
        let provider_name = provider.key().to_string();
        let mut provider_req = ProviderRequirement::default();

        match provider.expr() {
            hcl::Expression::Object(attr) => {
                if let Some(source) = attr.get(&ObjectKey::Identifier("source".into())) {
                    provider_req.source = source.to_string().replace('"', "");
                }
                if let Some(version) = attr.get(&ObjectKey::Identifier("version".into())) {
                    provider_req
                        .version_constraints
                        .push(version.to_string().replace('"', ""));
                }
            }
            _ => continue,
        };

        match module.required_providers.get_mut(&provider_name) {
            Some(existing_provider) => {
                if !provider_req.source.is_empty()
                    && !existing_provider.source.is_empty()
                    && existing_provider.source != provider_req.source
                {
                    return Err(ParseError::MultipleSourcesForProvider {
                        name: provider_name.clone(),
                        provider_source: existing_provider.source.clone(),
                        duplicate_source: provider_req.source.clone(),
                    });
                }

                existing_provider
                    .version_constraints
                    .append(&mut provider_req.version_constraints);
            }
            None => {
                _ = module
                    .required_providers
                    .insert(provider_name, provider_req)
            }
        };
    }

    Ok(())
}

fn get_files_in_dir(path: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut primary = vec![];
    let mut overrides = vec![];

    for entry in std::fs::read_dir(path)? {
        let file = entry?.path();
        if file.is_dir() {
            continue;
        }

        match file.extension() {
            Some(ext) => {
                match ext.to_str() {
                    Some(ext) => {
                        if ext.starts_with('.')
                            || ext.starts_with('#')
                            || ext.ends_with('~')
                            || ext.ends_with('#')
                        {
                            continue;
                        }
                    }
                    None => continue,
                };
            }
            None => continue,
        };

        let basename = match file.file_stem() {
            Some(basename) => basename.to_str().unwrap(),
            None => continue,
        };
        let is_override = basename == "override" || basename.ends_with("_override");

        if is_override {
            overrides.push(file);
        } else {
            primary.push(file);
        }
    }

    primary.append(&mut overrides);
    Ok(primary)
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs::File, io::Write};

    use tempdir::TempDir;

    use super::*;

    #[test]
    fn test_load_module() -> std::result::Result<(), Box<dyn Error>> {
        let tmp_dir = TempDir::new("test_load_module_from_file")?;
        let file_path = tmp_dir.path().join("version.tf");
        let mut file = File::create(file_path)?;
        file.write_all(
            r#"
            terraform {
                required_version = "1.0.0"

                required_providers {
                    mycloud = {
                        source  = "mycorp/mycloud"
                        version = "~> 1.0"
                    }
                }
            }
            "#
            .as_bytes(),
        )?;

        let pathbuf = tmp_dir.path().to_path_buf();
        let module = load_module(&pathbuf).unwrap();

        assert_eq!(module.required_core.len(), 1);
        assert_eq!(module.required_core[0], "1.0.0");

        assert_eq!(module.required_providers.len(), 1);
        let required_provider = module.required_providers.get("mycloud");
        assert!(required_provider.is_some());
        let required_provider = required_provider.unwrap();
        assert_eq!(required_provider.source, "mycorp/mycloud");
        assert_eq!(required_provider.version_constraints.len(), 1);
        assert_eq!(
            required_provider.version_constraints.first().unwrap(),
            "~> 1.0"
        );

        Ok(())
    }

    #[test]
    fn test_load_module_from_file() -> std::result::Result<(), Box<dyn Error>> {
        let file: hcl::Body = hcl::from_str(
            r#"
            terraform {
                required_version = "1.0.0"

                required_providers {
                    mycloud = {
                        source  = "mycorp/mycloud"
                        version = "~> 1.0"
                    }
                }
            }
            "#,
        )?;

        let pathbuf = PathBuf::from("");
        let mut module = Module::new(pathbuf);
        load_module_from_file(file, &mut module)?;

        assert_eq!(module.required_core.len(), 1);
        assert_eq!(module.required_core[0], "1.0.0");

        assert_eq!(module.required_providers.len(), 1);
        let required_provider = module.required_providers.get("mycloud");
        assert!(required_provider.is_some());
        let required_provider = required_provider.unwrap();
        assert_eq!(required_provider.source, "mycorp/mycloud");
        assert_eq!(required_provider.version_constraints.len(), 1);
        assert_eq!(
            required_provider.version_constraints.first().unwrap(),
            "~> 1.0"
        );

        Ok(())
    }

    #[test]
    fn test_load_module_from_file_with_multiple_sources() -> std::result::Result<(), Box<dyn Error>>
    {
        let file: hcl::Body = hcl::from_str(
            r#"
            terraform {
                required_version = "1.0.0"

                required_providers {
                    mycloud = {
                        source  = "mycorp/mycloud1"
                        version = "~> 1.0"
                    }
                    mycloud = {
                        source  = "mycorp/mycloud2"
                        version = "~> 2.0"
                    }
                }
            }
            "#,
        )?;

        let pathbuf = PathBuf::from("");
        let mut module = Module::new(pathbuf);
        let result = load_module_from_file(file, &mut module);

        assert_eq!(
            result,
            Err(ParseError::MultipleSourcesForProvider {
                name: "mycloud".to_string(),
                provider_source: "mycorp/mycloud1".to_string(),
                duplicate_source: "mycorp/mycloud2".to_string(),
            })
        );

        Ok(())
    }
}
