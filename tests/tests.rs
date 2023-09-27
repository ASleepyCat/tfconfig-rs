use std::{
    error::Error,
    fs::{self},
    path::PathBuf,
    result,
};
use tempdir::TempDir;
use tfconfig::{Error as TfConfigError, Module};

#[test]
fn test_load_module() -> result::Result<(), Box<dyn Error>> {
    let tmp_dir = TempDir::new("test_load_module_from_file")?;
    let tmp_dir_path = tmp_dir.path();
    let file_path = tmp_dir_path.join("version.tf");
    fs::write(
        file_path,
        r#"terraform {
        required_version = "1.0.0"

        required_providers {
            mycloud = {
                source  = "mycorp/mycloud"
                version = "~> 1.0"
            }
        }
    }"#,
    )?;

    let pathbuf = tmp_dir_path.to_path_buf();
    let module = tfconfig::load_module(&pathbuf, true)?;

    assert_eq!(1, module.required_core.len());
    assert_eq!(Some(&"1.0.0".to_string()), module.required_core.first());

    assert_eq!(1, module.required_providers.len());
    let required_provider = module.required_providers.get("mycloud");
    assert!(required_provider.is_some());
    let required_provider = required_provider.unwrap();
    assert_eq!("mycorp/mycloud", required_provider.source);
    assert_eq!(1, required_provider.version_constraints.len());
    assert_eq!(
        Some(&"~> 1.0".to_string()),
        required_provider.version_constraints.first()
    );

    Ok(())
}

#[test]
fn test_load_module_from_file() -> result::Result<(), Box<dyn Error>> {
    let file: hcl::Body = hcl::from_str(
        r#"terraform {
            required_version = "1.0.0"

            required_providers {
                mycloud = {
                    source  = "mycorp/mycloud"
                    version = "~> 1.0"
                }
            }
        }"#,
    )?;

    let pathbuf = PathBuf::from("");
    let mut module = Module::new(pathbuf.clone());
    tfconfig::load_module_from_file(&pathbuf, file, &mut module)?;

    assert_eq!(1, module.required_core.len());
    assert_eq!(Some(&"1.0.0".to_string()), module.required_core.first());

    assert_eq!(module.required_providers.len(), 1);
    let required_provider = module.required_providers.get("mycloud");
    assert!(required_provider.is_some());
    let required_provider = required_provider.unwrap();
    assert_eq!("mycorp/mycloud", required_provider.source);
    assert_eq!(1, required_provider.version_constraints.len());
    assert_eq!(
        Some(&"~> 1.0".to_string()),
        required_provider.version_constraints.first()
    );

    Ok(())
}

#[test]
fn test_load_module_from_file_unexpected_expr() -> result::Result<(), Box<dyn Error>> {
    let file: hcl::Body = hcl::from_str(
        r#"terraform {
            required_version = "1.0.0"

            required_providers {
                mycloud = "test"
            }
        }"#,
    )?;

    let pathbuf = PathBuf::from("test");
    let mut module = Module::new(pathbuf.clone());
    let result = tfconfig::load_module_from_file(&pathbuf, file, &mut module);
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
    let tmp_dir_path = tmp_dir.path();
    let good_file_path = tmp_dir_path.join("version.tf");
    fs::write(
        good_file_path,
        r#"terraform {
            required_version = "1.0.0"
            
            required_providers {
                mycloud = {
                    source  = "mycorp/mycloud"
                    version = "~> 1.0"
                }
            }
        }"#,
    )?;
    let bad_file_path = tmp_dir_path.join("bad.tf");
    fs::write(bad_file_path, "asdsadsadsad")?;

    let pathbuf = tmp_dir_path.to_path_buf();
    let module = tfconfig::load_module(&pathbuf, false)?;

    assert_eq!(1, module.required_core.len());
    assert_eq!("1.0.0", module.required_core[0]);

    assert_eq!(1, module.required_providers.len());
    let required_provider = module.required_providers.get("mycloud");
    assert!(required_provider.is_some());
    let required_provider = required_provider.unwrap();
    assert_eq!("mycorp/mycloud", required_provider.source);
    assert_eq!(1, required_provider.version_constraints.len());
    assert_eq!(
        Some(&"~> 1.0".to_string()),
        required_provider.version_constraints.first()
    );

    let res = tfconfig::load_module(&pathbuf, true);
    assert!(matches!(res, Err(TfConfigError::Parse(_))));

    Ok(())
}

#[test]
fn test_load_module_read_to_string_fail_not_strict() -> Result<(), Box<dyn Error>> {
    let tmp_dir = TempDir::new("test_load_module_read_to_string_fail")?;
    let tmp_dir_path = tmp_dir.path();
    let file_path = tmp_dir_path.join("version.tf");
    fs::write(file_path, vec![0xC3])?;

    tfconfig::load_module(tmp_dir_path, false)?;

    Ok(())
}

#[test]
fn test_load_module_read_to_string_fail_strict() -> Result<(), Box<dyn Error>> {
    let tmp_dir = TempDir::new("test_load_module_read_to_string_fail")?;
    let tmp_dir_path = tmp_dir.path();
    let file_path = tmp_dir_path.join("version.tf");
    fs::write(file_path, vec![0xC3])?;

    let res = tfconfig::load_module(tmp_dir_path, true);
    assert!(matches!(res, Err(TfConfigError::Io(_))));

    Ok(())
}
