
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hash;
use std::path::Path;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::PathBuf;

use tree_sitter::{Language, Parser};
use walkdir::WalkDir;

use crate::pipeline::SymbolsError;
use std::io::Write;

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
        }
    }
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase().replace("_", "")
}

const MIN_SYMBOL_NAME_LENGTH: usize = 4;

pub fn extract_symbols_for_language(
    parser: &mut Parser,
    source: &str,
) -> HashSet<String> {
    let mut symbols = HashSet::new();
    parser.set_timeout_micros(5_000_000);
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return symbols,
    };

    let mut stack = Vec::with_capacity(64);
    stack.push(tree.root_node());

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if (kind.contains("identifier") || kind.contains("name")) && node.end_byte() < source.as_bytes().len() {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let normalized = normalize_name(text);
                if !normalized.is_empty() && normalized.len() >= MIN_SYMBOL_NAME_LENGTH {
                    symbols.insert(normalized);
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

    symbols
}

pub fn find_symbols_local(root: &Path) -> Result<HashMap<SupportedLanguage, HashSet<String>>, Box<dyn std::error::Error>> {
    let mut parser_map: HashMap<SupportedLanguage, Parser> = HashMap::new();
    let mut symbols_map: HashMap<SupportedLanguage, HashSet<String>> = HashMap::new();

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

        let lang = match SupportedLanguage::from_path(&path) {
            Some(l) => l,
            None => continue,
        };

        let parser = parser_map.entry(lang).or_insert_with(|| {
            let mut p = Parser::new();
            p.set_language(&lang.ts_language())
                .expect("Failed to set Tree-sitter language");
            p
        });
        let src = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut symbols = extract_symbols_for_language(parser, &src);
        
        let out_set = symbols_map.entry(lang).or_insert_with(HashSet::new);
        for symbol in symbols.drain() {
            out_set.insert(symbol);
        }
    }

    Ok(symbols_map)
}

pub fn find_symbols(results_dir: &str, index: usize, root: &Path) -> Result<HashSet<SupportedLanguage>, Box<dyn std::error::Error>> {
    let mut parser_map: HashMap<SupportedLanguage, Parser> = HashMap::new();
    let mut file_count = 0;
    let mut symbol_count = 0;

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

        let lang = match SupportedLanguage::from_path(&path) {
            Some(l) => l,
            None => continue,
        };

        let parser = parser_map.entry(lang).or_insert_with(|| {
            let mut p = Parser::new();
            p.set_language(&lang.ts_language())
                .expect("Failed to set Tree-sitter language");
            p
        });
        let src = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut symbols = extract_symbols_for_language(parser, &src);
        
        let path_str = format!("{}/{}_{}_symbols.txt", results_dir, index, lang.to_string());
        let file = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path_str)?;

        let reader = BufReader::new(file.try_clone()?);
        for line in reader.lines() {
            if let Ok(line) = line {
                symbols.remove(&line);
            } else {
                break;
            }
        }
        
        let mut writer = BufWriter::new(file);
        for symbol in symbols.iter() {
            writeln!(writer, "{}", symbol)?;
            symbol_count += 1;
        }
        file_count += 1;
    }

    Ok(parser_map.keys().cloned().collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolData {
    pub name: String,
    pub parent_kind: Option<String>,
    pub grandparent_kind: Option<String>,
    pub great_grandparent_kind: Option<String>,
    pub path: String,
    pub start: usize,
}

pub fn extensive_extract_symbols_for_language(
    parser: &mut Parser,
    relative_path: String,
    lang: SupportedLanguage,
    source: &str,
    out: &mut HashSet<Box<SymbolData>>,
) {
    parser.set_timeout_micros(5_000_000);
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => {
            parser.reset();
            return;
        }
    };

    let mut stack = Vec::with_capacity(64);
    stack.push(tree.root_node());

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if kind.contains("identifier") {
            let parent = node.parent();
            let grandparent = parent.and_then(|p| p.parent());
            let great_grandparent = grandparent.and_then(|gp| gp.parent());

            if let Some(text) = node.utf8_text(source.as_bytes()).ok() {
                out.insert(Box::new(SymbolData {
                    name: text.to_string(),
                    parent_kind: parent.map(|p| p.kind().to_string()),
                    grandparent_kind: grandparent.map(|gp| gp.kind().to_string()),
                    great_grandparent_kind: great_grandparent.map(|ggp| ggp.kind().to_string()),
                    path: relative_path.clone(),
                    start: node.start_byte(),
                }));
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
    relative_path: String,
    parser_map: &mut HashMap<SupportedLanguage, Parser>,
    symbols_map: &mut HashMap<SupportedLanguage, HashSet<Box<SymbolData>>>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let src = fs::read_to_string(path)?;

    let lang = match SupportedLanguage::from_path(path) {
        Some(l) => l,
        None => return Ok(false),
    };

    let parser = parser_map.entry(lang).or_insert_with(|| {
        let mut p = Parser::new();
        p.set_language(&lang.ts_language())
            .expect("Failed to set Tree-sitter language");
        p
    });

    let out_set = symbols_map.entry(lang).or_insert_with(HashSet::new);

    // println!("Extracting for: {:?}, size: {}, language: {:?}", path, src.len(), lang);
    // if (lang == SupportedLanguage::Cpp || lang == SupportedLanguage::C) && src.len() > 200_000 {
    //     return Ok(false);
    // }
    extensive_extract_symbols_for_language(parser, relative_path, lang, &src, out_set);

    Ok(true)
}

pub fn extensive_find_symbols(
    root: &Path,
    per_file: usize,
    save: &dyn Fn(usize, HashMap<SupportedLanguage, HashSet<Box<SymbolData>>>) -> Result<(), SymbolsError>,
) -> Result<usize, SymbolsError> {
    let mut symbols_map: HashMap<SupportedLanguage, HashSet<Box<SymbolData>>> = HashMap::new();
    let mut parser_map: HashMap<SupportedLanguage, Parser> = HashMap::new();
    let mut symbols_count = 0;
    let mut index: usize = 0;

    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        if let Some(name) = e.file_name().to_str() {
            name != ".git" && name != "node_modules" && name != ".venv" && name != "__pycache__" && name != "vendor"
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

        let relative_path = match path.strip_prefix(root) {
            Ok(rel_path) => rel_path.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        match extensive_extract_symbols_for_file(&path, relative_path, &mut parser_map, &mut symbols_map) {
            Ok(true) => {},
            Ok(false) => continue,
            Err(_) => continue,
        }

        let size = symbols_map.values().map(|s| s.len()).sum::<usize>();
        if size >= per_file {
            save(index, symbols_map.clone())?;
            symbols_count += size;
            symbols_map.clear();
            index += 1;
        }
    }

    if !symbols_map.is_empty() {
        symbols_count += symbols_map.values().map(|s| s.len()).sum::<usize>();
        save(index, symbols_map)?;
        index += 1;
    }
    Ok(symbols_count)
}