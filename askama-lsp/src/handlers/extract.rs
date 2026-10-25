use std::{collections::HashSet, error::Error};

use quote::ToTokens;
use syn::{Fields, Item, ItemStruct, LitStr, spanned::Spanned};
use tower_lsp_server::ls_types::Uri;

use crate::structs::{Backend, DocumentBuffers, TemplateBuffers, TemplateFieldInfo};

impl Backend {
    pub fn parse_rust_file(
        &self,
        content: &str,
        file_uri: String,
        buffer: &mut TemplateBuffers,
    ) -> Result<(), Box<dyn Error>> {
        if let Ok(ast_tree) = syn::parse_file(&content) {
            for token in ast_tree.items {
                if let Item::Struct(struct_item) = token {
                    if let Fields::Named(fields_named) = &struct_item.fields {
                        for field in &fields_named.named {
                            if let Some(name) = &field.ident {
                                if let Some(template_path) =
                                    self.extract_template_path(&struct_item)
                                {
                                    let struct_name = &struct_item.ident;
                                    let field_info = TemplateFieldInfo {
                                        relative_template_path: template_path.to_string(),
                                        parent_struct: struct_name.to_string(),
                                        raw_type: field.ty.to_token_stream().to_string(),
                                        name: name.to_string(),
                                        line: field.span().start().line,
                                        character: field.span().start().column,
                                    };
                                    tracing::info!(
                                        "Success to extract template: {:#?}",
                                        field_info
                                    );
                                    if let Some(fields) = buffer.get_mut(&file_uri) {
                                        let mut hash = HashSet::new();
                                        fields.retain(|field| hash.insert(field.clone()));
                                    } else {
                                        buffer.insert(file_uri.clone(), vec![field_info]);
                                    }
                                } else {
                                    tracing::warn!("Cannot extract template path from struct");
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn extract_template_path(&self, item_struct: &ItemStruct) -> Option<String> {
        for attr in &item_struct.attrs {
            if attr.path().is_ident("template") {
                let mut template_path: Option<String> = None;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("path") {
                        let literal = meta.value()?.parse::<LitStr>()?;
                        let value = literal.value();
                        template_path = Some(value);
                        Ok(())
                    } else {
                        Err(meta.error("#[template(path = \"...\")] not found in struct"))
                    }
                });

                if let Some(template_path) = template_path {
                    return Some(template_path);
                }
            }
        }
        None
    }

    pub fn initialize_scan(
        _dirs: &[String],
        _template_buf: TemplateBuffers,
        _document_buf: DocumentBuffers,
    ) {
    }

    pub async fn collect_fields_info(&self, file_uri: &Uri, content: &str) {
        let file = file_uri.to_string();
        if file.ends_with(".rs") {
            tracing::info!("COLLECT FIELD");
            let mut field_cache_guard = self.buffers.fields_cache.write().await;
            if let Some(field_cache_by_uri) = field_cache_guard.get_mut(&file) {
                field_cache_by_uri.clear();
            }
            if let Err(err) = self.parse_rust_file(&content, file.clone(), &mut field_cache_guard) {
                tracing::error!("Faled to parse rust code from {}. Error: {}", file, err)
            }
        }
    }
}
