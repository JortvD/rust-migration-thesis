
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::io::Read;
use std::path::PathBuf;

use tokei::LanguageType;
use tree_sitter::{Language, Node, Parser};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
}

/// Known languages we support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportedLanguage {
    C,
    Go,
    Cpp,
    Csharp,
    Dart,
    Elixir,
    Haskell,
    Java,
    Javascript,
    OCaml,
    Lua,
    Python,
    Rust,
    Swift,
    Typescript,
    Tsx,
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        Self::from_extension(ext)
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(SupportedLanguage::Rust),
            "c" => Some(SupportedLanguage::C),
            "h" => Some(SupportedLanguage::C), // crude, but often OK
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some(SupportedLanguage::Cpp),
            "cs" => Some(SupportedLanguage::Csharp),
            "js" | "mjs" | "cjs" => Some(SupportedLanguage::Javascript),
            "ts" => Some(SupportedLanguage::Typescript),
            "tsx" => Some(SupportedLanguage::Tsx),
            "py" => Some(SupportedLanguage::Python),
            "go" => Some(SupportedLanguage::Go),
            "java" => Some(SupportedLanguage::Java),
            "swift" => Some(SupportedLanguage::Swift),
            "dart" => Some(SupportedLanguage::Dart),
            "ex" | "exs" => Some(SupportedLanguage::Elixir),
            "hs" => Some(SupportedLanguage::Haskell),
            "lua" => Some(SupportedLanguage::Lua),
            "ml" | "mli" => Some(SupportedLanguage::OCaml),
            _ => None,
        }
    }

    pub fn ts_language(self) -> Language {
        match self {
            SupportedLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
            SupportedLanguage::C => tree_sitter_c::LANGUAGE.into(),
            SupportedLanguage::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            SupportedLanguage::Csharp => tree_sitter_c_sharp::LANGUAGE.into(),
            SupportedLanguage::Javascript => tree_sitter_javascript::LANGUAGE.into(),
            SupportedLanguage::Typescript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            SupportedLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            SupportedLanguage::Python => tree_sitter_python::LANGUAGE.into(),
            SupportedLanguage::Go => tree_sitter_go::LANGUAGE.into(),
            SupportedLanguage::Java => tree_sitter_java::LANGUAGE.into(),
            SupportedLanguage::Swift => tree_sitter_swift::LANGUAGE.into(),
            SupportedLanguage::Dart => tree_sitter_dart_orchard::LANGUAGE.into(),
            SupportedLanguage::Elixir => tree_sitter_elixir::LANGUAGE.into(),
            SupportedLanguage::Haskell => tree_sitter_haskell::LANGUAGE.into(),
            SupportedLanguage::Lua => tree_sitter_lua::LANGUAGE.into(),
            SupportedLanguage::OCaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
        }
    }
    
    pub fn to_string(self) -> &'static str {
        match self {
            SupportedLanguage::Rust => "Rust",
            SupportedLanguage::C => "C",
            SupportedLanguage::Cpp => "C++",
            SupportedLanguage::Csharp => "C#",
            SupportedLanguage::Javascript => "JavaScript",
            SupportedLanguage::Typescript => "TypeScript",
            SupportedLanguage::Tsx => "TSX",
            SupportedLanguage::Python => "Python",
            SupportedLanguage::Go => "Go",
            SupportedLanguage::Java => "Java",
            SupportedLanguage::Swift => "Swift",
            SupportedLanguage::Dart => "Dart",
            SupportedLanguage::Elixir => "Elixir",
            SupportedLanguage::Haskell => "Haskell",
            SupportedLanguage::Lua => "Lua",
            SupportedLanguage::OCaml => "OCaml",
        }
    }
}

fn decl_kind_candidates(lang: SupportedLanguage) -> &'static [&'static str] {
    match lang {
        SupportedLanguage::Rust => &[
            "function_item",
            "impl_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "mod_item",
            "let_declaration",
            "const_item",
            "static_item",
            "type_item",
        ],
        SupportedLanguage::C | SupportedLanguage::Cpp => &[
            "function_definition",
            "declaration",
            "init_declarator",
            "field_declaration",
            "parameter_declaration",
            "function_declarator",
            "class_specifier",        // C++
            "struct_specifier",
            "union_specifier",
            "namespace_definition",   // C++
        ],
        SupportedLanguage::Csharp => &[
            "method_declaration",
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
            "field_declaration",
            "property_declaration",
            "variable_declarator",
            "parameter",
        ],
        SupportedLanguage::Javascript
        | SupportedLanguage::Typescript
        | SupportedLanguage::Tsx => &[
            "function_declaration",
            "function",
            "arrow_function",
            "method_definition",
            "class_declaration",
            "class",
            "lexical_declaration",
            "variable_declaration",
            "variable_declarator",
            "formal_parameter",
            "parameter",
            "function_signature",
            "interface_declaration",
            "type_alias_declaration",
            "enum_declaration",
            "assignment_expression",
        ],
        SupportedLanguage::Python => &[
            "function_definition",
            "class_definition",
            "assignment",
            "annassign",
            "typed_parameter",
            "for_statement",
            "with_item",
        ],
        SupportedLanguage::Go => &[
            "function_declaration",
            "method_declaration",
            "short_var_declaration",
            "var_declaration",
            "const_declaration",
            "type_declaration",
            "field_declaration",
            "parameter_declaration",
        ],
        SupportedLanguage::Java => &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "annotation_type_declaration",
            "method_declaration",
            "constructor_declaration",
            "field_declaration",
            "local_variable_declaration",
            "formal_parameter",
        ],
        SupportedLanguage::Swift => &[
            "function_declaration",
            "class_declaration",
            "struct_declaration",
            "enum_declaration",
            "protocol_declaration",
            "variable_declaration",
            "constant_declaration",
            "parameter_clause",
        ],
        SupportedLanguage::Dart => &[
            "function_declaration",
            "method_declaration",
            "class_declaration",
            "mixin_declaration",
            "extension_declaration",
            "variable_declaration",
            "field_declaration",
            "parameter",
        ],
        SupportedLanguage::Elixir => &[
            "function_definition",
            "module_definition",
            "variable_assignment",
            "parameter",
        ],
        SupportedLanguage::Haskell => &[
            "function_declaration",
            "data_declaration",
            "newtype_declaration",
            "type_declaration",
            "class_declaration",
            "instance_declaration",
        ],
        SupportedLanguage::Lua => &[
            "function_declaration",
            "local_variable_declaration",
            "variable_declaration",
        ],
        SupportedLanguage::OCaml => &[
            "function_definition",
            "type_definition",
            "module_definition",
            "let_binding",
        ],
    }
}

fn find_name_child<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if let Some(n) = node.child_by_field_name("name") {
        return Some(n);
    }

    if let Some(decl) = node.child_by_field_name("declarator") {
        if let Some(n) = decl.child_by_field_name("name") {
            return Some(n);
        }
        if may_be_identifier(decl.kind()) {
            return Some(decl);
        }
    }

    for i in 0..node.child_count() {
        let c = node.child(i).unwrap();
        if may_be_identifier(c.kind()) {
            return Some(c);
        }
    }

    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        for child in n.children(&mut n.walk()) {
            let k = child.kind();
            if may_be_identifier(k) {
                return Some(child);
            }
            
            stack.push(child);
        }
    }

    None
}

fn may_be_identifier(kind: &str) -> bool {
    kind.contains("identifier")
        || kind == "identifier"
        || kind == "type_identifier"
        || kind == "field_identifier"
        || kind == "property_identifier"
        || kind == "variable_name"
        || kind == "name"
        || kind == "module_name"
        || kind == "scoped_identifier"
        || kind == "attribute"
}

fn is_decl_kind(lang: SupportedLanguage, kind: &str) -> bool {
    for &candidate in decl_kind_candidates(lang) {
        if candidate == kind || kind.contains(candidate) || candidate.contains(kind) {
            return true;
        }
    }
    false
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase().replace("_", "")
}

pub fn extract_symbols_for_language(
    lang: SupportedLanguage,
    source: &str,
) -> Vec<Symbol> {
    let mut parser = Parser::new();
    parser
        .set_language(&lang.ts_language())
        .expect("Failed to set Tree-sitter language");

    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let root = tree.root_node();
    let mut symbols = Vec::new();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if is_decl_kind(lang, kind) {
            if let Some(name_node) = find_name_child(node) {
                if let Ok(text) = name_node.utf8_text(source.as_bytes()) {
                    symbols.push(Symbol {
                        name: normalize_name(text).clone(),
                    });
                }
            }
        }

        // DFS into children
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    symbols
}

pub fn extract_symbols_for_file(path: &Path) -> Option<Vec<Symbol>> {
    let mut file = fs::File::open(path).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    let ext = path.extension()?.to_str()?;
    let lang = SupportedLanguage::from_extension(ext)?;
    let result = extract_symbols_for_language(lang, &buf);
    drop(buf);
    Some(result)
}

pub fn find_symbols(root: &Path) -> Result<HashMap<SupportedLanguage, Vec<Symbol>>, Box<dyn std::error::Error>> {
    let mut symbols = HashMap::new();

    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        if let Some(name) = e.file_name().to_str() {
            name != ".git" && name != "target"
        } else {
            true
        }
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path: PathBuf = entry.path().into();

        if let Some(syms) = extract_symbols_for_file(&path) {
            let supported_lang = if let Some(lang) = SupportedLanguage::from_path(&path) {
                lang
            } else {
                continue;
            };

            symbols
                .entry(supported_lang)
                .or_insert_with(Vec::new)
                .extend(syms);
        }
    }

    Ok(symbols)
}