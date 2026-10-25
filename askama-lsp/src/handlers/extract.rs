use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use quote::ToTokens;
use syn::{Fields, Item, ItemStruct, LitStr, spanned::Spanned};
use tower_lsp_server::ls_types::Uri;
use tracing::{debug, trace, warn};

use crate::{
    config::AskamaLspConfig,
    structs::{Backend, TemplateBuffers, TemplateFieldInfo},
};

impl Backend {
    pub fn parse_rust_file(&self, content: &str) -> Vec<TemplateFieldInfo> {
        parse_template_fields(content)
    }

    fn workspace_roots(&self) -> Vec<PathBuf> {
        let mut roots = self
            .workspace_folders
            .lock()
            .unwrap()
            .iter()
            .filter_map(|folder| folder.uri.to_file_path().map(|path| path.into_owned()))
            .collect::<Vec<PathBuf>>();

        if roots.is_empty() {
            if let Ok(cwd) = std::env::current_dir() {
                roots.push(cwd);
            }
        }

        roots
    }

    pub async fn refresh_workspace_field_cache(&self) {
        let roots = self.workspace_roots();
        let config = self.config.read().await.clone();
        let mut result: TemplateBuffers = HashMap::new();

        debug!(
            workspace_roots = ?roots,
            source_directories = ?config.source_directories,
            "Refreshing Askama field cache"
        );

        for source_file in self.collect_source_files(roots.as_slice(), &config.source_directories) {
            match fs::read_to_string(&source_file) {
                Ok(content) => self.collect_rust_fields_for_content(
                    &content,
                    roots.as_slice(),
                    &config,
                    &mut result,
                ),
                Err(error) => warn!(
                    source_file = %source_file.display(),
                    %error,
                    "Failed to read Rust source file while building field cache"
                ),
            }
        }

        let docs = self.buffers.documents.read().await;
        for (uri, content) in docs.iter() {
            if uri.as_str().ends_with(".rs") {
                self.collect_rust_fields_for_content(
                    content,
                    roots.as_slice(),
                    &config,
                    &mut result,
                );
            }
        }

        for fields in result.values_mut() {
            let mut seen = HashSet::new();
            fields.retain(|field| seen.insert((field.parent_struct.clone(), field.name.clone())));
        }

        debug!(
            template_count = result.len(),
            field_count = result.values().map(Vec::len).sum::<usize>(),
            "Completed Askama field cache refresh"
        );

        let mut field_cache_guard = self.buffers.fields_cache.write().await;
        *field_cache_guard = result;
    }

    fn collect_rust_fields_for_content(
        &self,
        content: &str,
        roots: &[PathBuf],
        config: &AskamaLspConfig,
        result: &mut TemplateBuffers,
    ) {
        for field_info in self.parse_rust_file(content) {
            let template_uris = self.resolve_template_uris(
                field_info.relative_template_path.as_str(),
                roots,
                config,
            );

            trace!(
                template_path = %field_info.relative_template_path,
                discovered_template_uris = ?template_uris,
                field = %field_info.name,
                parent_struct = %field_info.parent_struct,
                "Mapping extracted Rust field into template cache"
            );

            for template_uri in template_uris {
                result
                    .entry(template_uri)
                    .or_default()
                    .push(field_info.clone());
            }
        }
    }

    fn collect_source_files(
        &self,
        roots: &[PathBuf],
        source_directories: &[String],
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();

        for root in roots {
            for source_dir in source_directories {
                let source_path = root.join(source_dir);
                self.collect_rust_files(source_path.as_path(), &mut files);
            }
        }

        files
    }

    fn collect_rust_files(&self, dir: &Path, files: &mut Vec<PathBuf>) {
        if !dir.exists() {
            return;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.collect_rust_files(path.as_path(), files);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    files.push(path);
                }
            }
        }
    }

    fn resolve_template_uris(
        &self,
        template_path: &str,
        roots: &[PathBuf],
        config: &AskamaLspConfig,
    ) -> Vec<String> {
        let mut uris = Vec::new();
        let template = Path::new(template_path);

        if template.is_absolute() {
            if let Some(uri) = Uri::from_file_path(normalize_path(template.to_path_buf())) {
                uris.push(uri.to_string());
            }
            return uris;
        }

        for root in roots {
            for template_dir in &config.templates {
                let full_path = normalize_path(root.join(template_dir).join(template_path));
                if let Some(uri) = Uri::from_file_path(full_path.as_path()) {
                    uris.push(uri.to_string());
                }
            }
        }

        uris
    }
}

fn parse_template_fields(content: &str) -> Vec<TemplateFieldInfo> {
    let mut fields = Vec::new();
    match syn::parse_file(content) {
        Ok(ast_tree) => {
            collect_template_fields(ast_tree.items.as_slice(), &mut fields);
            debug!(
                field_count = fields.len(),
                "Parsed Askama template fields from Rust content"
            );
        }
        Err(error) => {
            warn!(%error, "Failed to parse Rust source for Askama template field extraction");
        }
    }
    fields
}

fn collect_template_fields(items: &[Item], fields: &mut Vec<TemplateFieldInfo>) {
    for token in items {
        match token {
            Item::Struct(struct_item) => {
                if let Some(template_path) = extract_template_path(struct_item) {
                    trace!(
                        parent_struct = %struct_item.ident,
                        template_path = %template_path,
                        "Found Askama template struct"
                    );
                    extract_struct_fields(struct_item, template_path.as_str(), fields);
                }
            }
            Item::Mod(module) => {
                if let Some((_, module_items)) = &module.content {
                    collect_template_fields(module_items.as_slice(), fields);
                }
            }
            _ => {}
        }
    }
}

fn extract_struct_fields(
    struct_item: &ItemStruct,
    template_path: &str,
    fields: &mut Vec<TemplateFieldInfo>,
) {
    if let Fields::Named(fields_named) = &struct_item.fields {
        for field in &fields_named.named {
            if let Some(name) = &field.ident {
                fields.push(TemplateFieldInfo {
                    relative_template_path: template_path.to_string(),
                    parent_struct: struct_item.ident.to_string(),
                    raw_type: field.ty.to_token_stream().to_string(),
                    name: name.to_string(),
                    line: field.span().start().line,
                    character: field.span().start().column,
                });
            }
        }
    }
}

fn extract_template_path(item_struct: &ItemStruct) -> Option<String> {
    for attr in &item_struct.attrs {
        if attr.path().is_ident("template") {
            let mut template_path: Option<String> = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("path") {
                    let literal = meta.value()?.parse::<LitStr>()?;
                    template_path = Some(literal.value());
                }
                Ok(())
            });

            if let Some(template_path) = template_path {
                return Some(template_path);
            }
        }
    }
    None
}

fn normalize_path(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::parse_template_fields;

    #[test]
    fn extracts_template_fields_from_nested_module_structs() {
        let fields = parse_template_fields(
            r#"
            mod view {
                use askama::Template;

                #[derive(Template)]
                #[template(path = "index.html")]
                pub struct Page {
                    pub title: String,
                    pub visits: usize,
                }
            }
            "#,
        );

        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|field| field.name == "title"));
        assert!(fields.iter().any(|field| field.name == "visits"));
        assert!(
            fields
                .iter()
                .all(|field| field.relative_template_path == "index.html")
        );
    }
}
