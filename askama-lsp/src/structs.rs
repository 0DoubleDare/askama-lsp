use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::RwLock;
use tower_lsp_server::{
    Client,
    ls_types::{Uri, WorkspaceFolder},
};

use crate::config::AskamaLspConfig;

pub type TemplateBuffers = HashMap<String, Vec<TemplateFieldInfo>>;
pub type DocumentBuffers = HashMap<Uri, String>;

/// Информация для клиента: Родительская структура, тип поля,
/// имя поля, строка и символ на котором объявлено поле
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateFieldInfo {
    pub parent_struct: String,
    pub relative_template_path: String,
    pub raw_type: String,
    pub name: String,
    pub line: usize,
    pub character: usize,
}

#[derive(Default, Debug, Clone)]
pub struct Buffers {
    pub fields_cache: Arc<RwLock<TemplateBuffers>>,
    pub documents: Arc<RwLock<DocumentBuffers>>,
}

#[derive(Debug)]
pub struct Backend {
    pub config: Arc<RwLock<AskamaLspConfig>>,
    pub workspace_folders: Mutex<Vec<WorkspaceFolder>>,
    pub client: Client,
    pub buffers: Buffers,
}
