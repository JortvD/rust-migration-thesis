
use std::fs;
use std::path::Path;
use std::io::Read;
use std::path::PathBuf;

use tokei::LanguageType;
use tree_sitter::{Language, Node, Parser};
use walkdir::WalkDir;

pub fn is_code_language(lang: &LanguageType) -> bool {
    match lang {
        LanguageType::ActionScript => true,
        LanguageType::Ada => true,
        LanguageType::Agda => true,
        LanguageType::Alloy => true,
        LanguageType::Arduino => true,
        LanguageType::Asp => true,
        LanguageType::AspNet => true,
        LanguageType::Assembly => true,
        LanguageType::AssemblyGAS => true,
        LanguageType::AutoHotKey => true,
        LanguageType::Bash => true,
        LanguageType::Batch => true,
        LanguageType::BrightScript => true,
        LanguageType::C => true,
        LanguageType::CSharp => true,
        LanguageType::CShell => true,
        LanguageType::Clojure => true,
        LanguageType::ClojureC => true,
        LanguageType::ClojureScript => true,
        LanguageType::Cobol => true,
        LanguageType::CodeQL => true,
        LanguageType::CoffeeScript => true,
        LanguageType::Coq => true,
        LanguageType::Cpp => true,
        LanguageType::Crystal => true,
        LanguageType::D => true,
        LanguageType::Dart => true,
        LanguageType::DeviceTree => true,
        LanguageType::Elisp => true,
        LanguageType::Elixir => true,
        LanguageType::Elm => true,
        LanguageType::Elvish => true,
        LanguageType::Emojicode => true,
        LanguageType::Erlang => true,
        LanguageType::FSharp => true,
        LanguageType::Forth => true,
        LanguageType::FortranLegacy => true,
        LanguageType::FortranModern => true,
        LanguageType::Fstar => true,
        LanguageType::Futhark => true,
        LanguageType::GdScript => true,
        LanguageType::Gleam => true,
        LanguageType::Glsl => true,
        LanguageType::Go => true,
        LanguageType::Groovy => true,
        LanguageType::Gwion => true,
        LanguageType::Haskell => true,
        LanguageType::Haxe => true,
        LanguageType::Hlsl => true,
        LanguageType::HolyC => true,
        LanguageType::Idris => true,
        LanguageType::Isabelle => true,
        LanguageType::Jai => true,
        LanguageType::Java => true,
        LanguageType::JavaScript => true,
        LanguageType::Jsonnet => true,
        LanguageType::Jsx => true,
        LanguageType::Julia => true,
        LanguageType::Julius => true,
        LanguageType::K => true,
        LanguageType::KakouneScript => true,
        LanguageType::Kotlin => true,
        LanguageType::Lean => true,
        LanguageType::Lisp => true,
        LanguageType::LiveScript => true,
        LanguageType::Logtalk => true,
        LanguageType::Lua => true,
        LanguageType::Madlang => true,
        LanguageType::MoonScript => true,
        LanguageType::Nim => true,
        LanguageType::NotQuitePerl => true,
        LanguageType::OCaml => true,
        LanguageType::ObjectiveC => true,
        LanguageType::ObjectiveCpp => true,
        LanguageType::Odin => true,
        LanguageType::Oz => true,
        LanguageType::PSL => true,
        LanguageType::Pascal => true,
        LanguageType::Perl => true,
        LanguageType::Perl6 => true,
        LanguageType::Php => true,
        LanguageType::Pony => true,
        LanguageType::Processing => true,
        LanguageType::Prolog => true,
        LanguageType::PureScript => true,
        LanguageType::Python => true,
        LanguageType::Q => true,
        LanguageType::Qcl => true,
        LanguageType::Qml => true,
        LanguageType::R => true,
        LanguageType::Racket => true,
        LanguageType::Renpy => true,
        LanguageType::Ruby => true,
        LanguageType::Rust => true,
        LanguageType::Scala => true,
        LanguageType::Scheme => true,
        LanguageType::Sml => true,
        LanguageType::Solidity => true,
        LanguageType::SpecmanE => true,
        LanguageType::Spice => true,
        LanguageType::Sql => true,
        LanguageType::Stan => true,
        LanguageType::Stratego => true,
        LanguageType::Svelte => true,
        LanguageType::Swift => true,
        LanguageType::Swig => true,
        LanguageType::SystemVerilog => true,
        LanguageType::Tcl => true,
        LanguageType::Tsx => true,
        LanguageType::TypeScript => true,
        LanguageType::UnrealScript => true,
        LanguageType::Vala => true,
        LanguageType::Verilog => true,
        LanguageType::Vhdl => true,
        LanguageType::VimScript => true,
        LanguageType::VisualBasic => true,
        LanguageType::VB6 => true,
        LanguageType::VBScript => true,
        LanguageType::WebAssembly => true,
        LanguageType::Wolfram => true,
        LanguageType::Xtend => true,
        LanguageType::Zig => true,
        LanguageType::Zsh => true,
        _ => false,
    }
}


#[derive(Debug, Clone)]
pub struct Symbol {
    pub node_kind: String,
    pub name: String,
}

/// Known languages we support.
#[derive(Debug, Clone, Copy)]
pub enum SupportedLanguage {
    C,
    Go,
    Cpp,
    Csharp,
    Java,
    Javascript,
    Python,
    Rust,
    Swift,
    Typescript,
    Tsx,
}

impl SupportedLanguage {
    /// Map file extension -> SupportedLanguage
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
            _ => None,
        }
    }

    /// Get the Tree-sitter Language symbol.
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
            "class_specifier",   // C++
            "struct_specifier",
            "union_specifier",
            "namespace_definition", // C++
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
            "method_definition",
            "class_declaration",
            "class",
            "lexical_declaration",
            "variable_declaration",
            "variable_declarator",
            "formal_parameter",
            "function_signature",
            "interface_declaration",
            "type_alias_declaration",
            "enum_declaration",
        ],
        SupportedLanguage::Python => &[
            "function_definition",
            "class_definition",
            "assignment",
            "typed_parameter",
            "for_statement", // for target
            "with_item",
            "import_from_statement",
            "import_statement",
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
    }
}

fn is_decl_kind(lang: SupportedLanguage, kind: &str) -> bool {
    decl_kind_candidates(lang).iter().any(|k| *k == kind)
}

fn find_name_child<'a>(node: Node<'a>) -> Option<Node<'a>> {
    // 1. Preferred: field named "name" (very common)
    if let Some(n) = node.child_by_field_name("name") {
        return Some(n);
    }

    // 2. Some grammars use "declarator" which then contains an identifier
    if let Some(decl) = node.child_by_field_name("declarator") {
        if let Some(n) = decl.child_by_field_name("name") {
            return Some(n);
        }
        // fallback: look for identifiers inside declarator
        for i in 0..decl.child_count() {
            let c = decl.child(i).unwrap();
            let k = c.kind();
            if k.contains("identifier") || k == "identifier" || k == "type_identifier" {
                return Some(c);
            }
        }
    }

    // 3. Fallback: scan direct children for identifier-like kinds
    for i in 0..node.child_count() {
        let c = node.child(i).unwrap();
        let k = c.kind();
        if k.contains("identifier")
            || k == "identifier"
            || k == "type_identifier"
            || k == "field_identifier"
            || k == "property_identifier"
            || k == "variable_name"
        {
            return Some(c);
        }
    }

    None
}

pub fn extract_symbols_for_language(
    lang: SupportedLanguage,
    source: &str,
) -> Vec<Symbol> {
    let ts_lang: Language = lang.ts_language();

    let mut parser = Parser::new();
    parser
        .set_language(&ts_lang)
        .expect("Failed to set Tree-sitter language");

    let tree = match parser.parse(source, None) {
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
                        node_kind: kind.to_string(),
                        name: text.to_string().to_lowercase().replace("_", ""),
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

pub fn extract_symbols_for_path(path: &Path, source: &str) -> Option<Vec<Symbol>> {
    let ext = path.extension()?.to_str()?;
    let lang = SupportedLanguage::from_extension(ext)?;
    Some(extract_symbols_for_language(lang, source))
}

pub fn read_folder_symbols(root: &Path, match_language: Option<LanguageType>, not_match_language: Option<LanguageType>) -> Result<Vec<Symbol>, Box<dyn std::error::Error>> {
    let mut symbols = Vec::new();

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
        let language = LanguageType::from_file_extension(
            path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
        );

        if let Some(lang) = language {
            if !is_code_language(&lang) {
                continue;
            }

            if let Some(match_lang) = match_language {
                if lang != match_lang {
                    continue;
                }
            }

            if let Some(not_match_lang) = not_match_language {
                if lang == not_match_lang {
                    continue;
                }
            }
        } else {
            continue;
        }

        let mut file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            continue;
        }

        symbols.extend(extract_symbols_for_path(&path, &buf).unwrap_or_default());
    }

    Ok(symbols)
}

pub fn read_folder_symbols_per_language(root: &Path, match_language: Option<LanguageType>, not_match_language: Option<LanguageType>) -> Result<Vec<Symbol>, Box<dyn std::error::Error>> {
    let mut symbols = Vec::new();

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
        let language = LanguageType::from_file_extension(
            path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
        );

        if let Some(lang) = language {
            if !is_code_language(&lang) {
                continue;
            }

            if let Some(match_lang) = match_language {
                if lang != match_lang {
                    continue;
                }
            }

            if let Some(not_match_lang) = not_match_language {
                if lang == not_match_lang {
                    continue;
                }
            }
        } else {
            continue;
        }

        let mut file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            continue;
        }

        symbols.extend(extract_symbols_for_path(&path, &buf).unwrap_or_default());
    }

    Ok(symbols)
}