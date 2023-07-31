use hcl::ObjectKey;
use std::{
    collections::HashMap,
    error, fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Default)]
pub struct Module {
    pub path: PathBuf,
    pub required_core: Vec<String>,
    pub required_providers: HashMap<String, ProviderRequirement>,
}

impl Module {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            ..Default::default()
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

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Other(#[from] Box<dyn error::Error + Sync + Send>),
    #[error(transparent)]
    Parse(#[from] hcl::Error),
    #[error("unexpected expression for attribute {attribute_key:?} in {file_name}: {expr:?}")]
    UnexpectedExpr {
        attribute_key: String,
        expr: hcl::Expression,
        file_name: PathBuf,
    },
}

/// Reads the directory at the given path and attempts to interpret it as a Terraform module.
///
/// # Arguments
///
/// * `path` - Path to the directory containing the Terraform configuration
/// * `strict` - Whether to immediately return an error if a file in the directory cannot be parsed
pub fn load_module(path: &Path, strict: bool) -> Result<Module> {
    let mut module = Module::new(path.to_path_buf());

    let files = get_files_in_dir(path)?;

    for file_name in files {
        let file_contents = fs::read_to_string(&file_name)?;
        let file = match hcl::parse(&file_contents) {
            Ok(body) => body,
            Err(e) => match e {
                hcl::Error::Parse(e) => {
                    if strict {
                        return Err(Error::Parse(hcl::Error::Parse(e)));
                    } else {
                        continue;
                    }
                }
                _ => return Err(Error::Other(Box::new(e))),
            },
        };

        load_module_from_file(&file_name, file, &mut module)?;
    }

    Ok(module)
}

/// Reads given file, interprets it and stores in given [`Module`][Module]
pub fn load_module_from_file(
    current_file: &Path,
    file: hcl::Body,
    module: &mut Module,
) -> Result<()> {
    for block in file.blocks() {
        let body = block.body();

        #[allow(clippy::all)]
        match block.identifier() {
            "terraform" => handle_terraform_block(current_file, body, module)?,
            _ => (),
        }
    }

    Ok(())
}

fn handle_terraform_block(
    current_file: &Path,
    body: &hcl::Body,
    module: &mut Module,
) -> Result<()> {
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
            "required_providers" => {
                handle_required_providers_block(current_file, inner_block.body(), module)?
            }
            _ => (),
        }
    }

    Ok(())
}

fn handle_required_providers_block(
    current_file: &Path,
    required_providers: &hcl::Body,
    module: &mut Module,
) -> Result<()> {
    for provider in required_providers.attributes() {
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
            _ => {
                return Err(Error::UnexpectedExpr {
                    attribute_key: provider_name,
                    expr: provider.expr().clone(),
                    file_name: current_file.to_path_buf(),
                })
            }
        };

        module
            .required_providers
            .insert(provider_name, provider_req);
    }

    Ok(())
}

fn get_files_in_dir(path: &Path) -> Result<Vec<PathBuf>> {
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
