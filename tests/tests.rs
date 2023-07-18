use std::{error::Error, fs::File, io::Write, path::PathBuf, result};
use tempdir::TempDir;
use tfconfig::{Error as TfConfigError, Module};

#[test]
fn test_load_module() -> result::Result<(), Box<dyn Error>> {
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
    let module = tfconfig::load_module(&pathbuf, true)?;

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
fn test_load_module_from_file() -> result::Result<(), Box<dyn Error>> {
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
fn test_load_module_from_file_unexpected_expr() -> result::Result<(), Box<dyn Error>> {
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
    assert!(matches!(
        result,
        Err(TfConfigError::UnexpectedExpr {
            attribute_key,
            expr: _,
            file_name,
        }) if attribute_key == "mycloud" && file_name == pathbuf
    ));

    Ok(())
}

#[test]
fn test_load_module_strictness() -> result::Result<(), Box<dyn Error>> {
    let tmp_dir = TempDir::new("test_load_module_not_strict")?;
    let good_file_path = tmp_dir.path().join("version.tf");
    let mut good_file = File::create(good_file_path)?;
    good_file.write_all(
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
    let bad_file_path = tmp_dir.path().join("bad.tf");
    let mut bad_file = File::create(bad_file_path)?;
    bad_file.write_all("asdsadsadsad".as_bytes())?;

    let pathbuf = tmp_dir.path().to_path_buf();
    let module = tfconfig::load_module(&pathbuf, false)?;

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

    let res = tfconfig::load_module(&pathbuf, true);
    assert!(matches!(res, Err(TfConfigError::Parse(hcl::Error::Parse(_)))));

    Ok(())
}
