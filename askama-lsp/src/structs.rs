use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;
use tower_lsp_server::{Client, ls_types::Uri};

use crate::config::AskamaLspConfig;

pub type TemplateBuffers = HashMap<String, Vec<TemplateFieldInfo>>;
pub type DocumentBuffers = HashMap<Uri, String>;

/// Информация для клиента: Родительская структура, тип поля,
/// имя поля, строка и символ на котором объявлено поле
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateFieldInfo {
    pub parent_struct: String,
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

#[derive(Debug, Clone)]
pub struct Backend {
    pub config: AskamaLspConfig,
    pub client: Client,
    pub buffers: Buffers,
}
