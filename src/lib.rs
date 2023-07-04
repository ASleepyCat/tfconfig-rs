use std::{error::Error, fs, path::PathBuf};

#[derive(Debug)]
pub struct Module<'a> {
    pub path: &'a PathBuf,
    pub required_core: Vec<String>,
}

impl<'a> Module<'a> {
    fn new(path: &'a PathBuf) -> Self {
        Self {
            path,
            required_core: vec![],
        }
    }
}

/// Reads the directory at the given path and attempts to interpret it as a Terraform module.
pub fn load_module(path: &PathBuf) -> Result<Module, Box<dyn Error>> {
    let mut module = Module::new(path);

    let files = get_files_in_dir(path)?;

    for file_name in files {
        let file_contents = fs::read(&file_name)?;
        let file: hcl::Body = hcl::from_slice(&file_contents)?;

        load_module_from_file(file, &mut module);
    }

    Ok(module)
}

/// Reads given file, interprets it and stores in given [`Module`][Module]
pub fn load_module_from_file(file: hcl::Body, module: &mut Module) {
    file.blocks().for_each(|block| {
        let body = block.body();

        match block.identifier() {
            "terraform" => {
                body.attributes().for_each(|attr| match attr.key() {
                    "required_version" => {
                        module
                            .required_core
                            .push(attr.expr().to_string().replace('\"', ""));
                    }
                    _ => (),
                });
            }
            _ => (),
        }
    });
}

fn get_files_in_dir(path: &PathBuf) -> Result<Vec<String>, Box<dyn Error>> {
    let mut primary = vec![];
    let mut overrides = vec![];

    for entry in std::fs::read_dir(path)? {
        let file = entry?.path();
        if file.is_dir() {
            continue;
        }

        match file.extension() {
            Some(ext) => {
                let ext = ext.to_str().unwrap();
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

        let basename = match file.file_stem() {
            Some(basename) => basename.to_str().unwrap(),
            None => continue,
        };
        let is_override = basename == "override" || basename.ends_with("_override");

        if is_override {
            overrides.push(file.to_str().unwrap().to_string());
        } else {
            primary.push(file.to_str().unwrap().to_string());
        }
    }

    primary.append(&mut overrides);
    Ok(primary)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use tempdir::TempDir;

    use super::*;

    #[test]
    fn test_load_module() -> Result<(), Box<dyn Error>> {
        let tmp_dir = TempDir::new("test_load_module_from_file")?;
        let file_path = tmp_dir.path().join("version.tf");
        let mut file = File::create(file_path)?;
        file.write_all(r#"
        terraform {
            required_version = "1.0.0"
        }
        "#.as_bytes())?;

        let pathbuf = tmp_dir.path().to_path_buf();
        let module = load_module(&pathbuf).unwrap();

        assert_eq!(module.required_core.len(), 1);
        assert_eq!(module.required_core[0], "1.0.0");

        Ok(())
    }

    #[test]
    fn test_load_module_from_file() -> Result<(), Box<dyn Error>> {
        let file: hcl::Body = hcl::from_str(r#"
        terraform {
            required_version = "1.0.0"
        }
        "#)?;

        let pathbuf = PathBuf::from("");
        let mut module = Module::new(&pathbuf);
        load_module_from_file(file, &mut module);

        assert_eq!(module.required_core.len(), 1);
        assert_eq!(module.required_core[0], "1.0.0");

        Ok(())
    }
}