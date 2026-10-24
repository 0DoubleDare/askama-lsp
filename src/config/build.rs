use std::{error::Error, path::PathBuf};

use cargo_toml::Manifest;

use crate::config::AskamaLspConfig;

impl AskamaLspConfig {
    pub fn new(cargo_config: PathBuf) -> Result<Self, Box<dyn Error>> {
        let manifest = Manifest::from_path(cargo_config.as_os_str())?;

        if let Some(metadata) = manifest.package.and_then(|p| p.metadata) {
            if let Some(lsp_metadata) = metadata.get("askama-lsp") {
                let config: AskamaLspConfig = lsp_metadata
                    .clone()
                    .try_into()
                    .ok()
                    .unwrap_or_else(AskamaLspConfig::default);

                return Ok(config);
            }
        }

        Ok(Self::default())
    }
}
