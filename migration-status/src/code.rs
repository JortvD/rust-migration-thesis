
use std::collections::{HashMap, HashSet};
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
pub enum SupportedLanguage { // >500 stars (111k total)
    Bash, // 2.7k
    C, // 4.5k
    Clojure, // 288
    Cpp, // 6k
    Csharp, // 3.5k
    Dart, // 855
    Elixir, // 273
    Go, // 6.2k
    Haskell, // 218
    Java, // 7.1k
    Javascript, // 13.9k
    ObjectiveC, // 1.8k
    OCaml, // 83
    Perl, // 178
    Lua, // 712
    Nix, // 109
    PHP, // 2.9k
    Python, // 19.3k
    Ruby, // 2k
    Rust, // 3.2k
    Scala, // 285
    Swift,  // 2.3k
    Typescript, // 8.6k
    Tsx, // N/A
    // Groovy, // 73
    // SystemVerilog, // 24
    // PowerShell, // 371
    // Erlang, // 88
    // Emacs Lisp, // 277
    // Julia, // 118
    // R, // 269
    // CoffeeScript, // 156
    // 
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        Self::from_extension(ext)
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(SupportedLanguage::Rust),
            "c" | "h" => Some(SupportedLanguage::C),
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
            "rb" | "rbw" | "rake" => Some(SupportedLanguage::Ruby),
            "php" | "phtml" | "php3" | "php4" | "php5" | "php7" | "phps" => Some(SupportedLanguage::PHP),
            "nix" => Some(SupportedLanguage::Nix),
            "sh" | "bash" | "bashrc" | "bsh" => Some(SupportedLanguage::Bash),
            "scala" | "sc" => Some(SupportedLanguage::Scala),
            "objective-c" | "m" | "mm" => Some(SupportedLanguage::ObjectiveC),
            "clj" | "cljs" | "cljc" => Some(SupportedLanguage::Clojure),
            "pl" | "pm" => Some(SupportedLanguage::Perl),
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
            SupportedLanguage::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            SupportedLanguage::PHP => tree_sitter_php::LANGUAGE_PHP.into(),
            SupportedLanguage::Nix => tree_sitter_nix::LANGUAGE.into(),
            SupportedLanguage::Bash => tree_sitter_bash::LANGUAGE.into(),
            SupportedLanguage::Scala => tree_sitter_scala::LANGUAGE.into(),
            SupportedLanguage::ObjectiveC => tree_sitter_objc::LANGUAGE.into(),
            SupportedLanguage::Clojure => tree_sitter_clojure::LANGUAGE.into(),
            SupportedLanguage::Perl => tree_sitter_perl::LANGUAGE.into(),
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
            SupportedLanguage::Ruby => "Ruby",
            SupportedLanguage::PHP => "PHP",
            SupportedLanguage::Nix => "Nix",
            SupportedLanguage::Bash => "Bash",
            SupportedLanguage::Scala => "Scala",
            SupportedLanguage::ObjectiveC => "Objective-C",
            SupportedLanguage::Clojure => "Clojure",
            SupportedLanguage::Perl => "Perl",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KindGroup {
    Function,
    Type,
    Variable,
    Other,
}

impl KindGroup {
    pub fn to_string(self) -> &'static str {
        match self {
            KindGroup::Function => "Function",
            KindGroup::Type => "Type",
            KindGroup::Variable => "Variable",
            KindGroup::Other => "Other",
        }
    }
}

fn decl_kind_candidates(lang: SupportedLanguage) -> &'static [(&'static str, KindGroup)] {
    match lang {
        SupportedLanguage::Rust => &[
            ("function_item", KindGroup::Function),
            ("function_signature", KindGroup::Function),
            ("impl_item", KindGroup::Type),
            ("struct_item", KindGroup::Type),
            ("enum_item", KindGroup::Type),
            ("trait_item", KindGroup::Type),
            ("type_item", KindGroup::Type),
            ("macro_definition", KindGroup::Other),
            ("let_declaration", KindGroup::Variable),
            ("let_statement", KindGroup::Variable),
            ("const_item", KindGroup::Variable),
            ("static_item", KindGroup::Variable),
            ("mod_item", KindGroup::Other),
            ("use_declaration", KindGroup::Other),
        ],
        SupportedLanguage::C | SupportedLanguage::Cpp => &[
            ("function_definition", KindGroup::Function),
            ("function_declarator", KindGroup::Function),
            ("declarator", KindGroup::Variable),
            ("init_declarator", KindGroup::Variable),
            ("field_declaration", KindGroup::Variable),
            ("parameter_declaration", KindGroup::Variable),
            ("preproc_ifdef", KindGroup::Other),
            ("preproc_if", KindGroup::Other),
            ("preproc_define", KindGroup::Other),

            ("type_definition", KindGroup::Type),
            ("enum_specifier", KindGroup::Type),
            ("interface_specifier", KindGroup::Type),
            ("class_specifier", KindGroup::Type),   // C++
            ("struct_specifier", KindGroup::Type),  // C++
            ("union_specifier", KindGroup::Type),   // C++

            ("namespace_definition", KindGroup::Other), // C++
            ("declaration", KindGroup::Other),
            ("typedef_declaration", KindGroup::Type),
            ("macro_definition", KindGroup::Other),
        ],
        SupportedLanguage::Csharp => &[
            ("method_declaration", KindGroup::Function),
            ("function_definition", KindGroup::Function),
            ("constructor_declaration", KindGroup::Function),
            ("delegate_declaration", KindGroup::Type),
            ("class_declaration", KindGroup::Type),
            ("struct_declaration", KindGroup::Type),
            ("interface_declaration", KindGroup::Type),
            ("enum_declaration", KindGroup::Type),
            ("field_declaration", KindGroup::Variable),
            ("property_declaration", KindGroup::Variable),
            ("variable_declarator", KindGroup::Variable),
            ("local_declaration_statement", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("namespace_declaration", KindGroup::Other),
            ("attribute", KindGroup::Other),
        ],
        SupportedLanguage::Javascript
        | SupportedLanguage::Typescript
        | SupportedLanguage::Tsx => &[
            ("function_declaration", KindGroup::Function),
            ("function", KindGroup::Function),
            ("arrow_function", KindGroup::Function),
            ("method_definition", KindGroup::Function),
            ("method_signature", KindGroup::Function),

            ("class_declaration", KindGroup::Type),
            ("class", KindGroup::Type),
            ("interface_declaration", KindGroup::Type),
            ("interface", KindGroup::Type),
            ("type_alias_declaration", KindGroup::Type),
            ("enum_declaration", KindGroup::Type),
            ("object_type", KindGroup::Type),

            ("variable_declaration", KindGroup::Variable),
            ("variable_declarator", KindGroup::Variable),
            ("lexical_declaration", KindGroup::Variable),
            ("const_declaration", KindGroup::Variable),
            ("formal_parameter", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("formal_parameters", KindGroup::Variable),

            ("function_signature", KindGroup::Other),
        ],
        SupportedLanguage::Python => &[
            ("function_definition", KindGroup::Function),
            ("decorated_definition", KindGroup::Function),
            ("class_definition", KindGroup::Type),
            ("typed_parameter", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("assignment", KindGroup::Variable),
            ("augmented_assignment", KindGroup::Variable),
            ("for_statement", KindGroup::Other),
            ("with_statement", KindGroup::Other),
            ("with_item", KindGroup::Other),
            ("global_statement", KindGroup::Other),
        ],
        SupportedLanguage::Go => &[
            ("function_declaration", KindGroup::Function),
            ("method_declaration", KindGroup::Function),
            ("func_literal", KindGroup::Function),
            ("short_var_declaration", KindGroup::Variable),
            ("var_declaration", KindGroup::Variable),
            ("const_declaration", KindGroup::Variable),
            ("type_declaration", KindGroup::Type),
            ("type_spec", KindGroup::Type),
            ("field_declaration", KindGroup::Variable),
            ("parameter_declaration", KindGroup::Variable),
            ("receiver_parameter", KindGroup::Variable),
            ("interface_type", KindGroup::Type),
            ("package_clause", KindGroup::Other),
        ],
        SupportedLanguage::Java => &[
            ("class_declaration", KindGroup::Type),
            ("interface_declaration", KindGroup::Type),
            ("enum_declaration", KindGroup::Type),
            ("annotation_type_declaration", KindGroup::Type),
            ("method_declaration", KindGroup::Function),
            ("constructor_declaration", KindGroup::Function),
            ("field_declaration", KindGroup::Variable),
            ("variable_declarator", KindGroup::Variable),
            ("local_variable_declaration", KindGroup::Variable),
            ("formal_parameter", KindGroup::Variable),
            ("package_declaration", KindGroup::Other),
        ],
        SupportedLanguage::Swift => &[
            ("function_declaration", KindGroup::Function),
            ("method_declaration", KindGroup::Function),
            ("class_declaration", KindGroup::Type),
            ("struct_declaration", KindGroup::Type),
            ("enum_declaration", KindGroup::Type),
            ("protocol_declaration", KindGroup::Type),
            ("typealias_declaration", KindGroup::Type),
            ("variable_declaration", KindGroup::Variable),
            ("constant_declaration", KindGroup::Variable),
            ("parameter_clause", KindGroup::Variable),
            ("extension_declaration", KindGroup::Other),
        ],
        SupportedLanguage::Dart => &[
            ("function_declaration", KindGroup::Function),
            ("method_declaration", KindGroup::Function),
            ("class_declaration", KindGroup::Type),
            ("mixin_declaration", KindGroup::Type),
            ("extension_declaration", KindGroup::Type),
            ("typedef_declaration", KindGroup::Type),
            ("variable_declaration", KindGroup::Variable),
            ("field_declaration", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("constructor_declaration", KindGroup::Function),
        ],
        SupportedLanguage::Elixir => &[
            ("function_definition", KindGroup::Function),
            ("module_definition", KindGroup::Type),
            ("attribute", KindGroup::Other),
            ("variable_assignment", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("alias", KindGroup::Other),
        ],
        SupportedLanguage::Haskell => &[
            ("function_declaration", KindGroup::Function),
            ("pattern_binding", KindGroup::Variable),
            ("data_declaration", KindGroup::Type),
            ("newtype_declaration", KindGroup::Type),
            ("type_declaration", KindGroup::Type),
            ("class_declaration", KindGroup::Type),
            ("instance_declaration", KindGroup::Other),
            ("type_signature", KindGroup::Other),
        ],
        SupportedLanguage::Lua => &[
            ("function_declaration", KindGroup::Function),
            ("function_definition", KindGroup::Function),
            ("local_variable_declaration", KindGroup::Variable),
            ("variable_declaration", KindGroup::Variable),
            ("assignment_statement", KindGroup::Variable),
            ("local_statement", KindGroup::Variable),
            ("block", KindGroup::Other),
        ],
        SupportedLanguage::OCaml => &[
            ("function_definition", KindGroup::Function),
            ("value_binding", KindGroup::Variable),
            ("let_binding", KindGroup::Variable),
            ("type_definition", KindGroup::Type),
            ("module_definition", KindGroup::Type),
            ("module_binding", KindGroup::Type),
            ("external_declaration", KindGroup::Other),
        ],
        SupportedLanguage::Ruby => &[
            ("method", KindGroup::Function),
            ("class", KindGroup::Type),
            ("module", KindGroup::Type),
            ("constant", KindGroup::Variable),
            ("assignment", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("singleton_method", KindGroup::Function),
            ("module_function", KindGroup::Function),
            ("singleton_class", KindGroup::Type),
            ("block", KindGroup::Other),
            ("method_call", KindGroup::Function),
            ("symbol_literal", KindGroup::Variable),
        ],
        SupportedLanguage::PHP => &[
            ("function_definition", KindGroup::Function),
            ("method_declaration", KindGroup::Function),
            ("class_declaration", KindGroup::Type),
            ("interface_declaration", KindGroup::Type),
            ("trait_declaration", KindGroup::Type),
            ("variable_name", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("constant_declaration", KindGroup::Variable),
            ("namespace_definition", KindGroup::Other),
        ],
        SupportedLanguage::Nix => &[
            ("function", KindGroup::Function),
            ("let_in", KindGroup::Other),
            ("attribute_set", KindGroup::Other),
            ("identifier", KindGroup::Variable),
        ],
        SupportedLanguage::Bash => &[
            ("function_definition", KindGroup::Function),
            ("variable_assignment", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("command", KindGroup::Other),
        ],
        SupportedLanguage::Scala => &[
            ("function_definition", KindGroup::Function),
            ("method_definition", KindGroup::Function),
            ("class_definition", KindGroup::Type),
            ("object_definition", KindGroup::Type),
            ("trait_definition", KindGroup::Type),
            ("variable_declaration", KindGroup::Variable),
            ("value_declaration", KindGroup::Variable),
            ("parameter", KindGroup::Variable),
            ("package_declaration", KindGroup::Other),
        ],
        SupportedLanguage::ObjectiveC => &[
            ("function_definition", KindGroup::Function),
            ("method_declaration", KindGroup::Function),
            ("interface_declaration", KindGroup::Type),
            ("implementation_declaration", KindGroup::Type),
            ("protocol_declaration", KindGroup::Type),
            ("variable_declaration", KindGroup::Variable),
            ("parameter_declaration", KindGroup::Variable),
            ("property_declaration", KindGroup::Variable),
            ("category_declaration", KindGroup::Other),
        ],
        SupportedLanguage::Clojure => &[
            ("function_definition", KindGroup::Function),
            ("def", KindGroup::Variable),
            ("defn", KindGroup::Function),
            ("defmacro", KindGroup::Function),
            ("deftype", KindGroup::Type),
            ("defrecord", KindGroup::Type),
            ("defprotocol", KindGroup::Type),
            ("ns", KindGroup::Other),
        ],
        SupportedLanguage::Perl => &[
            ("subroutine_definition", KindGroup::Function),
            ("package_definition", KindGroup::Type),
            ("package_declaration", KindGroup::Type),
            ("variable_declaration", KindGroup::Variable),
            ("variable_assignment", KindGroup::Variable),
            ("package_declaration", KindGroup::Other),
            ("use_statement", KindGroup::Other),
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
}

fn find_decl_kind(lang: SupportedLanguage, kind: &str) -> Option<KindGroup> {
    for &candidate in decl_kind_candidates(lang) {
        if candidate.0 == kind || kind.contains(candidate.0) || candidate.0.contains(kind) {
            return Some(candidate.1);
        }
    }
    None
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase().replace("_", "")
}

const MIN_SYMBOL_NAME_LENGTH: usize = 4;

pub fn extract_symbols_for_language(
    parser: &mut Parser,
    lang: SupportedLanguage,
    source: &str,
    out: &mut HashSet<String>,
) {
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return,
    };

    let mut stack = Vec::with_capacity(64);
    stack.push(tree.root_node());

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if let Some(_) = find_decl_kind(lang, kind) {
            if let Some(name_node) = find_name_child(node) {
                if let Ok(text) = name_node.utf8_text(source.as_bytes()) {
                    let normalized = normalize_name(text);
                    if !normalized.is_empty() && normalized.len() >= MIN_SYMBOL_NAME_LENGTH {
                        out.insert(normalized);
                    }
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
    parser.reset();
}

pub fn extract_symbols_for_file(
    path: &Path,
    parser_map: &mut HashMap<SupportedLanguage, Parser>,
    symbols_map: &mut HashMap<SupportedLanguage, HashSet<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = fs::read_to_string(path)?;

    let lang = match SupportedLanguage::from_path(path) {
        Some(l) => l,
        None => return Ok(()),
    };

    let parser = parser_map.entry(lang).or_insert_with(|| {
        let mut p = Parser::new();
        p.set_language(&lang.ts_language())
            .expect("Failed to set Tree-sitter language");
        p
    });

    let out_set = symbols_map.entry(lang).or_insert_with(HashSet::new);

    extract_symbols_for_language(parser, lang, &src, out_set);

    Ok(())
}

pub fn find_symbols(root: &Path) -> Result<(HashMap<SupportedLanguage, HashSet<String>>, usize), Box<dyn std::error::Error>> {
    let mut symbols_map: HashMap<SupportedLanguage, HashSet<String>> = HashMap::new();
    let mut parser_map: HashMap<SupportedLanguage, Parser> = HashMap::new();
    let mut file_count = 0;

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

        if path.extension().is_none() {
            continue;
        }

        if let Err(_) = extract_symbols_for_file(&path, &mut parser_map, &mut symbols_map) {
            continue;
        }
        file_count += 1;
    }

    Ok((symbols_map, file_count))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolData {
    pub name: String,
    pub kind: String,
    pub name_kind: String,
    pub group: KindGroup,
}

pub fn extensive_extract_symbols_for_language(
    parser: &mut Parser,
    lang: SupportedLanguage,
    source: &str,
    out: &mut HashSet<SymbolData>,
) {
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return,
    };

    let mut stack = Vec::with_capacity(64);
    stack.push(tree.root_node());

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if let Some(group) = find_decl_kind(lang, kind) {
            if let Some(name_node) = find_name_child(node) {
                if let Ok(text) = name_node.utf8_text(source.as_bytes()) {
                    out.insert(SymbolData {
                        name: text.to_string(),
                        kind: kind.to_string(),
                        name_kind: name_node.kind().to_string(),
                        group,
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
    parser.reset();
}

pub fn extensive_extract_symbols_for_file(
    path: &Path,
    parser_map: &mut HashMap<SupportedLanguage, Parser>,
    symbols_map: &mut HashMap<SupportedLanguage, HashSet<SymbolData>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = fs::read_to_string(path)?;

    let lang = match SupportedLanguage::from_path(path) {
        Some(l) => l,
        None => return Ok(()),
    };

    let parser = parser_map.entry(lang).or_insert_with(|| {
        let mut p = Parser::new();
        p.set_language(&lang.ts_language())
            .expect("Failed to set Tree-sitter language");
        p
    });

    let out_set = symbols_map.entry(lang).or_insert_with(HashSet::new);

    extensive_extract_symbols_for_language(parser, lang, &src, out_set);

    Ok(())
}

pub fn extensive_find_symbols(root: &Path) -> Result<(HashMap<SupportedLanguage, HashSet<SymbolData>>, usize), Box<dyn std::error::Error>> {
    let mut symbols_map: HashMap<SupportedLanguage, HashSet<SymbolData>> = HashMap::new();
    let mut parser_map: HashMap<SupportedLanguage, Parser> = HashMap::new();
    let mut file_count = 0;

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

        if path.extension().is_none() {
            continue;
        }

        if let Err(_) = extensive_extract_symbols_for_file(&path, &mut parser_map, &mut symbols_map) {
            continue;
        }
        file_count += 1;
    }

    Ok((symbols_map, file_count))
}