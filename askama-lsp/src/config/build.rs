use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use cargo_toml::Manifest;

use crate::config::{AskamaLspConfig, AskamaLspFileConfig, PartialAskamaLspConfig};

impl AskamaLspConfig {
    pub fn new(cargo_config: PathBuf) -> Result<Self, Box<dyn Error>> {
        let mut config = Self::default();
        config.merge_partial(Self::from_cargo_manifest(cargo_config.as_path())?);
        Ok(config)
    }

    pub fn from_workspace_roots(workspace_roots: &[PathBuf]) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();

        let roots: Vec<PathBuf> = if workspace_roots.is_empty() {
            std::env::current_dir().ok().into_iter().collect()
        } else {
            workspace_roots.to_vec()
        };

        for root in roots {
            let manifest_path = root.join("Cargo.toml");
            if manifest_path.exists() {
                match Self::from_cargo_manifest(&manifest_path) {
                    Ok(partial) => config.merge_partial(partial),
                    Err(err) => warnings.push(format!(
                        "Failed to read askama-lsp metadata from {}: {}",
                        manifest_path.display(),
                        err
                    )),
                }
            }

            if let Some(config_path) = Self::find_config_file(&root) {
                match Self::from_config_file(config_path.as_path()) {
                    Ok(partial) => config.merge_partial(partial),
                    Err(err) => warnings.push(format!(
                        "Failed to parse {}: {}",
                        config_path.display(),
                        err
                    )),
                }
            }
        }

        (config, warnings)
    }

    fn from_cargo_manifest(cargo_config: &Path) -> Result<PartialAskamaLspConfig, Box<dyn Error>> {
        let manifest = Manifest::from_path(cargo_config.as_os_str())?;

        if let Some(metadata) = manifest.package.and_then(|p| p.metadata) {
            if let Some(lsp_metadata) = metadata.get("askama-lsp") {
                let config: PartialAskamaLspConfig = lsp_metadata
                    .clone()
                    .try_into()
                    .map_err(|err| format!("Invalid askama-lsp metadata: {err}"))?;

                return Ok(config);
            }
        }

        Ok(PartialAskamaLspConfig::default())
    }

    fn from_config_file(path: &Path) -> Result<PartialAskamaLspConfig, Box<dyn Error>> {
        let content = fs::read_to_string(path)?;
        let config: AskamaLspFileConfig = toml::from_str(content.as_str())?;
        Ok(config.into_partial())
    }

    fn find_config_file(root: &Path) -> Option<PathBuf> {
        let primary = root.join("askama-lsp.toml");
        if primary.exists() {
            return Some(primary);
        }

        let hidden = root.join(".askama-lsp.toml");
        if hidden.exists() {
            return Some(hidden);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::AskamaLspConfig;

    fn test_dir(name: &str) -> PathBuf {
        let unique = format!(
            "askama_lsp_config_{}_{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn merges_cargo_metadata_with_defaults() {
        let root = test_dir("metadata");
        let manifest_path = root.join("Cargo.toml");

        fs::write(
            &manifest_path,
            r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"

[package.metadata.askama-lsp]
templates = ["./views"]
"#,
        )
        .unwrap();

        let config = AskamaLspConfig::new(manifest_path).unwrap();

        assert_eq!(config.templates, vec!["./views"]);
        assert_eq!(
            config.templates_extensions,
            vec!["html".to_string(), "askama".to_string(), "j2".to_string()]
        );
        assert_eq!(config.source_directories, vec!["./src"]);
    }

    #[test]
    fn workspace_config_file_overrides_manifest() {
        let root = test_dir("workspace");

        fs::write(
            root.join("Cargo.toml"),
            r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"

[package.metadata.askama-lsp]
templates = ["./manifest_templates"]
source_directories = ["./manifest_src"]
"#,
        )
        .unwrap();

        fs::write(
            root.join("askama-lsp.toml"),
            r#"[askama-lsp]
templates = ["./templates_custom"]
source_directories = ["./src_custom"]
"#,
        )
        .unwrap();

        let (config, warnings) = AskamaLspConfig::from_workspace_roots(&[root]);

        assert!(warnings.is_empty());
        assert_eq!(config.templates, vec!["./templates_custom"]);
        assert_eq!(config.source_directories, vec!["./src_custom"]);
    }

    #[test]
    fn invalid_config_file_returns_warning_and_keeps_defaults() {
        let root = test_dir("invalid");
        fs::write(root.join("askama-lsp.toml"), "templates = [").unwrap();

        let (config, warnings) = AskamaLspConfig::from_workspace_roots(&[root]);

        assert_eq!(config, AskamaLspConfig::default());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Failed to parse"));
    }
}
