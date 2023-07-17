use std::{error::Error, fs::File, io::Write, path::PathBuf};
use tempdir::TempDir;
use tfconfig::{Module, Error as TfConfigError};

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
    let module = tfconfig::load_module(&pathbuf)?;

    assert_eq!(module.required_core.len(), 1);
    assert_eq!(module.required_core[0], "1.0.0");

    assert_eq!(module.required_providers.len(), 1);
    let required_provider = module.required_providers.get("mycloud");
    assert!(required_provider.is_some());
    let required_provider = required_provider.unwrap();
    assert_eq!(required_provider.source, "mycorp/mycloud");
    assert_eq!(required_provider.version_constraints.len(), 1);
    assert_eq!(
        required_provider.version_constraints.first(),
        Some(&"~> 1.0".to_string())
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
    let mut module = Module::new(pathbuf.clone());
    tfconfig::load_module_from_file(&pathbuf, file, &mut module)?;

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
fn test_load_module_from_file_unexpected_expr() -> std::result::Result<(), Box<dyn Error>> {
    let file: hcl::Body = hcl::from_str(
        r#"
        terraform {
            required_version = "1.0.0"

            required_providers {
                mycloud = "test"
            }
        }
        "#,
    )?;

    let pathbuf = PathBuf::from("test");
    let mut module = Module::new(pathbuf.clone());
    let result = tfconfig::load_module_from_file(&pathbuf, file, &mut module);

    assert!(result.is_err());
    if let TfConfigError::UnexpectedExpr { attribute_key, expr: _, file_name } = result.unwrap_err() {
        assert_eq!("mycloud", attribute_key);
        assert_eq!(pathbuf, file_name);
    } else {
        panic!("Unexpected error type");
    }

    Ok(())
}
