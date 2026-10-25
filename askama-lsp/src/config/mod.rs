pub mod build;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AskamaLspConfig {
    pub templates: Vec<String>,
    pub templates_extensions: Vec<String>,
    pub source_directories: Vec<String>,
}

#[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct PartialAskamaLspConfig {
    pub templates: Option<Vec<String>>,
    pub templates_extensions: Option<Vec<String>>,
    pub source_directories: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct AskamaLspFileConfig {
    #[serde(rename = "askama-lsp")]
    pub askama_lsp: Option<PartialAskamaLspConfig>,
    pub templates: Option<Vec<String>>,
    pub templates_extensions: Option<Vec<String>>,
    pub source_directories: Option<Vec<String>>,
}

impl Default for AskamaLspConfig {
    fn default() -> Self {
        Self {
            templates: vec!["./templates".to_string()],
            templates_extensions: vec!["html".to_string(), "askama".to_string(), "j2".to_string()],
            source_directories: vec!["./src".to_string()],
        }
    }
}

impl AskamaLspConfig {
    pub fn merge_partial(&mut self, partial: PartialAskamaLspConfig) {
        if let Some(templates) = partial.templates {
            self.templates = templates;
        }
        if let Some(templates_extensions) = partial.templates_extensions {
            self.templates_extensions = templates_extensions;
        }
        if let Some(source_directories) = partial.source_directories {
            self.source_directories = source_directories;
        }
    }
}

impl AskamaLspFileConfig {
    pub fn into_partial(self) -> PartialAskamaLspConfig {
        if let Some(config) = self.askama_lsp {
            return config;
        }

        PartialAskamaLspConfig {
            templates: self.templates,
            templates_extensions: self.templates_extensions,
            source_directories: self.source_directories,
        }
    }
}
