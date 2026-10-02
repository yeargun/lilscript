//! Editor semantics over the same configured graph, including unsaved imports.
use super::*;
use lilscript::module::SourceOverride;
use lilscript::{configured_entries, CheckedProgram, GraphSession};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct GraphClient<'a> {
    session: GraphSession<'a>,
    pub reset: bool,
}
impl<'a> GraphClient<'a> {
    pub fn new(arena: &'a Bump) -> Self {
        Self {
            session: GraphSession::new(arena),
            reset: false,
        }
    }

    fn inspect<R>(
        &mut self,
        uri: &str,
        documents: &HashMap<String, Document>,
        client: impl for<'v> FnOnce(&CheckedProgram<'v, 'a, 'a>, usize) -> R,
    ) -> Result<Option<R>, String> {
        let Some(path) = file_uri_path(uri).filter(|path| path.is_file()) else {
            return Ok(None);
        };
        let config = load_project_config(&path, None)
            .map_err(|error| error.to_string())?
            .config;
        let entries = configured_entries(&path, &config);
        let buffers = buffers(documents);
        let overrides = buffers
            .iter()
            .map(|(path, source)| SourceOverride { path, source })
            .collect::<Vec<_>>();
        let canonical = path.canonicalize().map_err(|error| error.to_string())?;
        let result = self
            .session
            .inspect(&entries, &overrides, &path, &config, |checked| {
                checked
                    .modules
                    .modules
                    .iter()
                    .position(|module| module.path == canonical)
                    .map(|module| client(checked, module))
            })
            .map_err(|error| error.to_string());
        self.reset |= self.session.needs_new_epoch(&config);
        result
    }

    /// One check per configured entry graph, followed by diagnostics for every
    /// open member. Editing/closing a dependency therefore refreshes its users.
    pub fn publish(
        &mut self,
        connection: &Connection,
        documents: &HashMap<String, Document>,
        focus: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let buffers = buffers(documents);
        let overrides = buffers
            .iter()
            .map(|(path, source)| SourceOverride { path, source })
            .collect::<Vec<_>>();
        let mut groups = BTreeSet::new();
        let mut uris = documents.keys().collect::<Vec<_>>();
        uris.sort();
        for uri in uris {
            let document = &documents[uri];
            let Some(path) = file_uri_path(uri).filter(|path| path.is_file()) else {
                publish(connection, uri, document, diagnostics(None, &document.text))?;
                continue;
            };
            let loaded = match load_project_config(&path, None) {
                Ok(loaded) => loaded,
                Err(error) => {
                    publish(
                        connection,
                        uri,
                        document,
                        vec![error_diagnostic(
                            &document.text,
                            Span::empty(0),
                            error.to_string(),
                        )],
                    )?;
                    continue;
                }
            };
            let entries = configured_entries(&path, &loaded.config);
            if !groups.insert(format!("{:?}:{entries:?}", loaded.path)) {
                continue;
            }
            let selected = file_uri_path(focus).unwrap_or_else(|| path.clone());
            let result =
                self.session
                    .inspect(&entries, &overrides, &selected, &loaded.config, |checked| {
                        let findings = match lint_checked(checked, &loaded.config) {
                            Ok(findings) => findings,
                            Err(error) => {
                                return BTreeMap::from([(
                                    path.clone(),
                                    vec![error_diagnostic(
                                        &document.text,
                                        Span::empty(0),
                                        error.to_string(),
                                    )],
                                )])
                            }
                        };
                        let mut by_path = BTreeMap::<PathBuf, Vec<LintDiagnostic>>::new();
                        for finding in findings {
                            by_path
                                .entry(finding.path.clone())
                                .or_default()
                                .push(finding);
                        }
                        checked
                            .modules
                            .modules
                            .iter()
                            .map(|module| {
                                (
                                    module.path.clone(),
                                    lint_diagnostics(
                                        &module.path,
                                        module.source,
                                        by_path.remove(&module.path).unwrap_or_default(),
                                    ),
                                )
                            })
                            .collect::<BTreeMap<_, _>>()
                    });
            self.reset |= self.session.needs_new_epoch(&loaded.config);
            match result {
                Ok(findings) => {
                    for (other_uri, other) in documents {
                        if let Some(path) =
                            file_uri_path(other_uri).and_then(|path| path.canonicalize().ok())
                        {
                            if let Some(diagnostics) = findings.get(&path) {
                                publish(connection, other_uri, other, diagnostics.clone())?;
                            }
                        }
                    }
                }
                Err(error) => {
                    // Report on each open document belonging to this configured
                    // graph, so a dependency's error cannot leave stale success.
                    for (other_uri, other) in documents {
                        let Some(other_path) = file_uri_path(other_uri) else {
                            continue;
                        };
                        let same = load_project_config(&other_path, None).ok().is_some_and(
                            |other_config| {
                                other_config.path == loaded.path
                                    && configured_entries(&other_path, &other_config.config)
                                        == entries
                            },
                        );
                        if same {
                            publish(
                                connection,
                                other_uri,
                                other,
                                vec![compiler_diagnostic(Some(&other_path), &other.text, &error)],
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn completion(&mut self, params: &Value, documents: &HashMap<String, Document>) -> Value {
        let uri = request_uri(params).unwrap_or("");
        let source = documents.get(uri).map(|document| document.text.as_str());
        let mut result = completion_result(source);
        let details = self.inspect(uri, documents, |checked, module| {
            let view = checked.semantics.view(module).unwrap();
            let mut details = BTreeMap::new();
            lilscript::ast_walk::each_identifier(&checked.syntax[module], &mut |ident| {
                if let Some(ty) = view.binding_type(ident.id) {
                    details
                        .entry(ident.name.to_owned())
                        .or_insert_with(|| ty.to_string());
                }
            });
            details
        });
        if let Ok(Some(details)) = details {
            if let Some(items) = result["items"].as_array_mut() {
                for item in items {
                    if let Some(detail) =
                        item["label"].as_str().and_then(|label| details.get(label))
                    {
                        item["detail"] = json!(detail);
                    }
                }
            }
        }
        result
    }

    pub fn hover(&mut self, params: &Value, documents: &HashMap<String, Document>) -> Value {
        let Some((document, line, character)) = document_position(params, documents) else {
            return Value::Null;
        };
        let Some((word, span)) = byte_offset(&document.text, line, character)
            .and_then(|offset| word_at(&document.text, offset))
        else {
            return Value::Null;
        };
        let detail = self.inspect(
            request_uri(params).unwrap_or(""),
            documents,
            |checked, module| {
                let view = checked.semantics.view(module).unwrap();
                let mut detail = None;
                lilscript::ast_walk::each_identifier(&checked.syntax[module], &mut |ident| {
                    if ident.span == span {
                        detail = view
                            .binding_type(ident.id)
                            .or_else(|| {
                                view.identifier_symbol(ident.id).map(|symbol| {
                                    &checked.semantics.symbols()[symbol.0 as usize].ty
                                })
                            })
                            .map(ToString::to_string)
                            .or(detail.take());
                    }
                });
                detail.or_else(|| {
                    view.type_binding(word)
                        .map(|id| format!("type {}", view.nominal_name(id).unwrap_or(word)))
                })
            },
        );
        if let Ok(Some(Some(detail))) = detail {
            json!({"contents":{"kind":"markdown","value":format!("```lilscript\n{detail} {word}\n```")},"range":span_range(&document.text, span)})
        } else {
            hover_result(params, documents)
        }
    }

    pub fn references(&mut self, params: &Value, documents: &HashMap<String, Document>) -> Value {
        let Some((document, line, character)) = document_position(params, documents) else {
            return json!([]);
        };
        let Some((_, span)) = byte_offset(&document.text, line, character)
            .and_then(|offset| word_at(&document.text, offset))
        else {
            return json!([]);
        };
        self.inspect(request_uri(params).unwrap_or(""),documents,|checked,module| {
            let Some(symbol) = symbol_at(checked,module,span) else { return json!([]); };
            let mut locations = Vec::new();
            for (owner, syntax) in checked.syntax.iter().enumerate() {
                let view = checked.semantics.view(owner).unwrap();
                let input = &checked.modules.modules[owner];
                let mut spans = BTreeSet::new();
                lilscript::ast_walk::each_identifier(syntax,&mut |ident| {
                    if view.identifier_symbol(ident.id)==Some(symbol) {spans.insert((ident.span.start,ident.span.end));}
                });
                for (start,end) in spans {locations.push(json!({"uri":path_uri(&input.path),"range":span_range(input.source,Span::new(start,end))}));}
            }
            Value::Array(locations)
        }).ok().flatten().unwrap_or_else(|| references_result(params,documents))
    }

    pub fn rename(
        &mut self,
        params: &Value,
        documents: &HashMap<String, Document>,
    ) -> Result<Value, (i32, String)> {
        let failure = |message: &str| (ErrorCode::InvalidParams as i32, message.to_owned());
        let name =
            string_at(params, "/newName").ok_or_else(|| failure("rename requires newName"))?;
        if !valid_identifier(name) || keyword_name(name) {
            return Err(failure("newName must be a LilScript identifier"));
        }
        let (document, line, character) = document_position(params, documents)
            .ok_or_else(|| failure("rename position is outside the open document"))?;
        let (word, span) = byte_offset(&document.text, line, character)
            .and_then(|offset| word_at(&document.text, offset))
            .ok_or_else(|| failure("select an identifier"))?;
        let result=self.inspect(request_uri(params).unwrap_or(""),documents,|checked,module| {
            let symbol=symbol_at(checked,module,span).ok_or_else(||failure("select a checked value binding"))?;
            let source = &checked.syntax[module];
            let view = checked.semantics.view(module).unwrap();
            let imported = source.imports.iter().flat_map(|import| import.specifiers).find(|specifier|
                specifier.local.name==word && view.identifier_symbol(specifier.local.id)==Some(symbol));
            let foreign = source.foreign_imports.iter().flat_map(|import| import.specifiers).find(|specifier|
                specifier.local.name==word && view.identifier_symbol(specifier.local.id)==Some(symbol));
            if checked.semantics.symbols()[symbol.0 as usize].is_foreign() && foreign.is_none() {
                return Err(failure("a host global's external name cannot be renamed inside this graph"));
            }
            let alias = imported.is_some_and(|specifier| specifier.local.span!=specifier.imported.span)
                || foreign.is_some() || checked.semantics.symbols()[symbol.0 as usize].name!=word;
            if checked.semantics.symbols().iter().any(|existing| existing.id!=symbol && existing.name==name) {
                return Err(failure("the requested name already belongs to a binding in this graph"));
            }
            let mut changes=serde_json::Map::new();
            for (owner,syntax) in checked.syntax.iter().enumerate() {
                if alias && owner!=module {continue;}
                let view=checked.semantics.view(owner).unwrap();
                let input=&checked.modules.modules[owner];
                let mut edits=BTreeMap::new();
                let preserves_alias = !alias && syntax.imports.iter().flat_map(|import| import.specifiers).any(|specifier|
                    specifier.local.name==word && specifier.local.span!=specifier.imported.span && view.identifier_symbol(specifier.local.id)==Some(symbol));
                lilscript::ast_walk::each_identifier(syntax,&mut |ident| {
                    if !preserves_alias && ident.name==word && view.identifier_symbol(ident.id)==Some(symbol) {
                        edits.insert((ident.span.start,ident.span.end),name.to_owned());
                    }
                });
                for specifiers in syntax.imports.iter().map(|import|import.specifiers).chain(syntax.foreign_imports.iter().map(|import|import.specifiers)) {
                    for specifier in specifiers {
                        if view.identifier_symbol(specifier.local.id)!=Some(symbol) {continue;}
                        if alias {
                            if specifier.local.name==word && specifier.local.span==specifier.imported.span {
                                edits.insert((specifier.local.span.start,specifier.local.span.end),format!("{} as {name}",specifier.imported.name));
                            }
                        } else if specifier.imported.name==word {
                            edits.insert((specifier.imported.span.start,specifier.imported.span.end),name.to_owned());
                        }
                    }
                }
                if !edits.is_empty() { changes.insert(path_uri(&input.path),Value::Array(edits.into_iter().map(|((start,end),name)|
                    json!({"range":span_range(input.source,Span::new(start,end)),"newText":name})).collect())); }
            }
            Ok(json!({"changes":changes}))
        }).map_err(|message| (ErrorCode::InvalidParams as i32,message))?;
        result.unwrap_or_else(|| rename_result(params, documents))
    }
}

fn buffers(documents: &HashMap<String, Document>) -> BTreeMap<PathBuf, &str> {
    documents
        .iter()
        .filter_map(|(uri, document)| {
            let path = file_uri_path(uri)?.canonicalize().ok()?;
            Some((path, document.text.as_str()))
        })
        .collect()
}
fn publish(
    connection: &Connection,
    uri: &str,
    document: &Document,
    diagnostics: Vec<Value>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    send_notification(
        connection,
        "textDocument/publishDiagnostics",
        json!({"uri":uri,"version":document.version,"diagnostics":diagnostics}),
    )
}
fn symbol_at(
    checked: &CheckedProgram<'_, '_, '_>,
    module: usize,
    span: Span,
) -> Option<lilscript::check::SymbolId> {
    let view = checked.semantics.view(module)?;
    let mut result = None;
    lilscript::ast_walk::each_identifier(&checked.syntax[module], &mut |ident| {
        if ident.span == span {
            result = view.identifier_symbol(ident.id).or(result);
        }
    });
    result
}
fn path_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Work {
        path: PathBuf,
        root: String,
        helper: String,
        documents: HashMap<String, Document>,
    }
    impl Work {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("lilscript-d3-editor-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                path.join("lilscript.toml"),
                "[delivery.entries]\nmain='main.lil'\n",
            )
            .unwrap();
            let main =
                "import {plus as added} from \"./helper\";export int answer(){return added(3);}";
            let dependency = "export int plus(int n){return n+1;}";
            std::fs::write(path.join("main.lil"), main).unwrap();
            std::fs::write(path.join("helper.lil"), dependency).unwrap();
            let root = path_uri(&path.join("main.lil"));
            let helper = path_uri(&path.join("helper.lil"));
            let documents = [
                (
                    root.clone(),
                    Document {
                        text: main.into(),
                        version: Some(1),
                    },
                ),
                (
                    helper.clone(),
                    Document {
                        text: dependency.into(),
                        version: Some(2),
                    },
                ),
            ]
            .into_iter()
            .collect();
            Self {
                path,
                root,
                helper,
                documents,
            }
        }
        fn params(&self, uri: &str, word: &str) -> Value {
            let offset = self.documents[uri].text.rfind(word).unwrap();
            json!({"textDocument":{"uri":uri},"position":position_at(&self.documents[uri].text,offset)})
        }
        fn apply(&mut self, changes: &Value) {
            for (uri, edits) in changes["changes"].as_object().unwrap() {
                let document = self.documents.get_mut(uri).unwrap();
                let mut edits = edits
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|edit| {
                        let start = &edit["range"]["start"];
                        let end = &edit["range"]["end"];
                        (
                            byte_offset(
                                &document.text,
                                start["line"].as_u64().unwrap() as u32,
                                start["character"].as_u64().unwrap() as u32,
                            )
                            .unwrap(),
                            byte_offset(
                                &document.text,
                                end["line"].as_u64().unwrap() as u32,
                                end["character"].as_u64().unwrap() as u32,
                            )
                            .unwrap(),
                            edit["newText"].as_str().unwrap(),
                        )
                    })
                    .collect::<Vec<_>>();
                edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
                for (start, end, text) in edits {
                    document.text.replace_range(start..end, text);
                }
            }
        }
    }
    impl Drop for Work {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    #[test]
    fn d3_editor_hover_completion_and_references_use_imported_bindings() {
        let work = Work::new("queries space");
        let arena = Bump::new();
        let mut graph = GraphClient::new(&arena);
        let params = work.params(&work.root, "added");
        let hover = graph.hover(&params, &work.documents);
        assert!(
            hover["contents"]["value"].as_str().unwrap().contains("int"),
            "{hover}"
        );
        let completion = graph.completion(&params, &work.documents);
        let added = completion["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["label"] == "added")
            .unwrap();
        assert!(added["detail"].as_str().unwrap().contains("int"), "{added}");
        let refs = graph.references(&params, &work.documents);
        let refs = refs.as_array().unwrap();
        assert!(refs.iter().any(|item| item["uri"] == work.helper));
        assert!(refs.iter().any(|item| item["uri"] == work.root));
    }
    #[test]
    fn d3_editor_rename_preserves_aliases_and_updates_cross_module_imports() {
        let mut work = Work::new("rename");
        let arena = Bump::new();
        let mut graph = GraphClient::new(&arena);
        let mut params = work.params(&work.root, "added");
        params["newName"] = json!("increment");
        let edit = graph.rename(&params, &work.documents).unwrap();
        assert_eq!(edit["changes"].as_object().unwrap().len(), 1);
        work.apply(&edit);
        assert!(work.documents[&work.root]
            .text
            .contains("plus as increment"));
        let mut params = work.params(&work.helper, "plus");
        params["newName"] = json!("sum");
        let edit = graph.rename(&params, &work.documents).unwrap();
        assert_eq!(edit["changes"].as_object().unwrap().len(), 2);
        work.apply(&edit);
        assert!(work.documents[&work.root].text.contains("sum as increment"));
        assert!(graph
            .inspect(&work.root, &work.documents, |_, _| ())
            .unwrap()
            .is_some());
    }

    #[test]
    fn d3_editor_keeps_explicit_same_name_and_foreign_import_aliases() {
        let mut work = Work::new("alias-boundaries");
        let arena = Bump::new();
        let mut graph = GraphClient::new(&arena);
        work.documents.get_mut(&work.root).unwrap().text =
            "import {plus as plus} from \"./helper\";export int answer(){return plus(3);}".into();
        let mut params = work.params(&work.helper, "plus");
        params["newName"] = json!("sum");
        let edit = graph.rename(&params, &work.documents).unwrap();
        work.apply(&edit);
        assert!(work.documents[&work.root].text.contains("sum as plus"));
        assert!(graph
            .inspect(&work.root, &work.documents, |_, _| ())
            .unwrap()
            .is_some());
        work.documents.get_mut(&work.root).unwrap().text="import extern {read} from \"external\";extern int read();export int answer(){return read();}".into();
        let mut params = work.params(&work.root, "read");
        params["newName"] = json!("localRead");
        let edit = graph.rename(&params, &work.documents).unwrap();
        work.apply(&edit);
        assert!(work.documents[&work.root]
            .text
            .contains("read as localRead"));
        assert!(graph
            .inspect(&work.root, &work.documents, |_, _| ())
            .unwrap()
            .is_some());
    }
    #[test]
    fn d3_editor_dependency_edits_and_close_refresh_all_graph_diagnostics() {
        let mut work = Work::new("diagnostics");
        let arena = Bump::new();
        let mut graph = GraphClient::new(&arena);
        let (server, client) = Connection::memory();
        graph.publish(&server, &work.documents, &work.root).unwrap();
        let initial = client.receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(initial.len(), 2);
        work.documents.get_mut(&work.helper).unwrap().text =
            "export string plus(int n){return \"wrong\";}".into();
        graph
            .publish(&server, &work.documents, &work.helper)
            .unwrap();
        let changed = client.receiver.try_iter().collect::<Vec<_>>();
        assert!(changed.iter().any(|message|matches!(message,Message::Notification(note) if note.params["uri"]==work.root && !note.params["diagnostics"].as_array().unwrap().is_empty())),"{changed:?}");
        work.documents.remove(&work.helper);
        graph
            .publish(&server, &work.documents, &work.helper)
            .unwrap();
        let restored = client.receiver.try_iter().collect::<Vec<_>>();
        assert!(restored.iter().any(|message|matches!(message,Message::Notification(note) if note.params["uri"]==work.root && note.params["diagnostics"].as_array().unwrap().is_empty())),"{restored:?}");
    }
}
