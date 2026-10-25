pub mod build;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AskamaLspConfig {
    templates: Vec<String>,
    templates_extensions: Vec<String>,
    source_directories: Vec<String>,
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
