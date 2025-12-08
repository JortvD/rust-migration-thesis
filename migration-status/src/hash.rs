use std::{fs::{self, File}, hash::BuildHasherDefault, io::{BufRead, BufReader, Write, Read}, path::PathBuf};

use flate2::bufread::GzDecoder;
use fnv::FnvHasher;
use probminhash::superminhasher2::{SuperMinHash2, get_jaccard_index_estimate};

#[derive(Clone)]
struct Identifier {
	language: String,
	path: String,
	start: usize,
	name: String,
	parent_kind: Option<String>,
	grandparent_kind: Option<String>,
	great_grandparent_kind: Option<String>,
}

fn parse_identifier(line: &str) -> Option<Identifier> {
	let tokens: Vec<&str> = line.split(',').collect();
	if tokens.len() < 7 {
		return None;
	}
	Some(Identifier {
		language: tokens[0].to_string(),
		path: tokens[1].to_string(),
		start: tokens[2].parse().unwrap_or(0),
		name: tokens[3].to_string(),
		parent_kind: if tokens[4].is_empty() { None } else { Some(tokens[4].to_string()) },
		grandparent_kind: if tokens[5].is_empty() { None } else { Some(tokens[5].to_string()) },
		great_grandparent_kind: if tokens[6].is_empty() { None } else { Some(tokens[6].to_string()) },
	})
}

fn get_file_identifiers(file_path: &PathBuf) -> Vec<Identifier> {
    let file = File::open(file_path).expect("Failed to open file");
    let reader = BufReader::new(file);
    let decoder = BufReader::new(GzDecoder::new(reader));

    decoder.lines()
        .filter_map(|line| line.ok().and_then(|l| parse_identifier(&l)))
        .collect()
}

fn identifier_match(identifier: &Identifier, kinds: &[&str]) -> bool {
    match identifier.parent_kind.as_deref() {
        Some(parent) => kinds.contains(&parent),
        None => false,
    }
}

fn keep_identifier(identifier: &Identifier) -> bool {
	return true;
    match identifier.language.as_str() {
        "Rust" => identifier_match(identifier, &[
            "function_item",
            "impl_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "type_item",
            "macro_definition",
            "let_declaration",
            "const_item",
            "static_item",
            "mod_item",
            "field_identifier",
        ]),
        "C" | "C++" => identifier_match(identifier, &[
            "function_definition",
            "function_declarator", // The actual name part of the function
            "declarator",          // Variable names
            "init_declarator",     // int x = 1;
            "field_declaration",   // struct fields
            "parameter_declaration",
            "preproc_define",      // #define MACRO
            "type_definition",
            "enum_specifier",
            "class_specifier",
            "struct_specifier",
            "union_specifier",
            "namespace_definition",
            "typedef_declaration",
        ]),
        "C#" => identifier_match(identifier, &[
            "method_declaration",
            "constructor_declaration",
            "delegate_declaration",
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
            "field_declaration",
            "property_declaration",
            "variable_declarator",
            "parameter",
            "namespace_declaration",
        ]),
        "JavaScript" | "TypeScript" | "TSX" => identifier_match(identifier, &[
            "function_declaration",
            "function_expression", // var x = function y() {} -> y
            "arrow_function",
            "method_definition",
            "class_declaration",
            "interface_declaration",
            "type_alias_declaration",
            "enum_declaration",
            "variable_declarator", // const x = 1
            "formal_parameter",
            "property_signature",  // TS interface props
            "public_field_definition",
        ]),
        "Python" => identifier_match(identifier, &[
            "function_definition",
            "class_definition",
            "parameter",
            "typed_parameter",
            "assignment", 
            "global_statement",
        ]),
        "Go" => identifier_match(identifier, &[
            "function_declaration",
            "method_declaration",
            "short_var_declaration", // x := 1
            "var_declaration",       // var x int
            "const_declaration",
            "type_declaration",
            "field_declaration",
            "parameter_declaration",
            "package_clause",
        ]),
        "Java" => identifier_match(identifier, &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "method_declaration",
            "constructor_declaration",
            "field_declaration",
            "variable_declarator",
            "formal_parameter",
            "package_declaration",
        ]),
        "Swift" => identifier_match(identifier, &[
            "function_declaration",
            "class_declaration",
            "struct_declaration",
            "enum_declaration",
            "protocol_declaration",
            "typealias_declaration",
            "variable_declaration", // var x
            "constant_declaration", // let x
            "parameter",
            "initializer_declaration",
        ]),
        "Dart" => identifier_match(identifier, &[
            "function_declaration",
            "method_declaration",
            "class_declaration",
            "mixin_declaration",
            "extension_declaration",
            "typedef_declaration",
            "variable_declaration",
            "field_declaration",
            "parameter",
            "constructor_declaration",
        ]),
        "Elixir" => identifier_match(identifier, &[
            "function_definition", // def/defp
            "module_definition",   // defmodule
            "variable_assignment", // pattern matching assignment
            "parameter",
        ]),
        "Haskell" => identifier_match(identifier, &[
            "function_declaration", // Type signature `foo :: Int`
            "function_definition",  // Implementation `foo = ...`
            "data_declaration",
            "newtype_declaration",
            "type_declaration",
            "class_declaration",
            "variable",             // In patterns
        ]),
        "Lua" => identifier_match(identifier, &[
            "function_declaration",
            "local_variable_declaration",
            "assignment_statement", // x = 1
            "parameter",
        ]),
        "OCaml" => identifier_match(identifier, &[
            "value_binding",        // let x = ...
            "type_definition",
            "module_definition",
            "external_declaration",
            "parameter",
        ]),
        "Ruby" => identifier_match(identifier, &[
            "method",
            "class",
            "module",
            "assignment",           // x = 1
            "constant_assignment",  // CONST = 1
            "parameter",
            "singleton_method",
        ]),
        "PHP" => identifier_match(identifier, &[
            "function_definition",
            "method_declaration",
            "class_declaration",
            "interface_declaration",
            "trait_declaration",
            "assignment_expression", // $x = 1
            "parameter",
            "constant_declaration",
            "namespace_definition",
        ]),
        "Nix" => identifier_match(identifier, &[
            "function",       // arg: body
            "bind",           // x = y; (inside let or set)
            "attrpath",       // { x = 1; }
        ]),
        "Bash" => identifier_match(identifier, &[
            "function_definition",
            "variable_assignment",
        ]),
        "Scala" => identifier_match(identifier, &[
            "function_definition",
            "class_definition",
            "object_definition",
            "trait_definition",
            "val_definition",
            "var_definition",
            "parameter",
        ]),
        "Objective-C" => identifier_match(identifier, &[
            "function_definition",
            "method_declaration",
            "interface_declaration",
            "implementation_declaration",
            "protocol_declaration",
            "type_definition",
            "property_declaration",
            "parameter_declaration",
        ]),
        "Clojure" => identifier_match(identifier, &[
            "list_lit",
            "def_lit",  // (def x ...)
        ]),
        "Perl" => identifier_match(identifier, &[
            "subroutine_declaration",
            "package_statement",
            "variable_declaration", // my $x
            "parameter",
        ]),
        _ => false,
    }
}

const MIN_LENGTH: usize = 10;

fn filter_identifiers(identifiers: Vec<Identifier>) -> Vec<Identifier> {
	identifiers.into_iter()
		.filter(|id| keep_identifier(id) && id.name.len() >= MIN_LENGTH)
		.collect()
}

fn normalize_and_deduplicate_identifiers(identifiers: Vec<Identifier>) -> Vec<String> {
	let mut unique_identifiers = std::collections::HashSet::new();

	for identifier in identifiers {
		let normalized: String = identifier.name.chars()
            .filter(|c| *c != '_')
            .map(|c| c.to_ascii_lowercase())
            .collect();
		unique_identifiers.insert(normalized);
	}

	unique_identifiers.into_iter().collect()
}

const CHUNK_SIZE: usize = 1_000_000;

pub fn hash_symbols_file(
	file_path: PathBuf,
	output: String
) {
	let project = file_path.parent().unwrap().file_stem().unwrap().to_str().unwrap();
	let index = file_path.file_stem().unwrap().to_str().unwrap().split('_').nth(1).unwrap_or("0").replace(".csv", "");

	let read_start_time = std::time::Instant::now();
	let identifiers = get_file_identifiers(&file_path);
	let read_duration = read_start_time.elapsed();
	let identifiers_count = identifiers.len();
	println!("[{}][{}] Read {} identifiers in {} ms.", project, index, identifiers_count, read_duration.as_millis());

	for (i, chunk) in identifiers.chunks(CHUNK_SIZE).enumerate() {
		let identifiers_count = chunk.len();
		let filter_start_time = std::time::Instant::now();
		let filtered_identifiers = filter_identifiers(chunk.to_vec());
		let mut writer = fs::OpenOptions::new()
		.append(true)
		.create(true)
		.open(format!("{}/test/{}_{}_{}_identifiers.csv", "results", project, index, i))
		.unwrap();
		for identifier in &filtered_identifiers {
			writer.write_all(format!("{},{},{},{},{},{},{}\n",
				identifier.language,
				identifier.path,
				identifier.start,
				identifier.name,
				identifier.parent_kind.as_deref().unwrap_or(""),
				identifier.grandparent_kind.as_deref().unwrap_or(""),
				identifier.great_grandparent_kind.as_deref().unwrap_or(""),
			).as_bytes()).unwrap();
		}
		let normalized_identifiers = normalize_and_deduplicate_identifiers(filtered_identifiers);
		let filter_duration = filter_start_time.elapsed();
		let symbol_count = normalized_identifiers.len();
		
		let hash_start_time = std::time::Instant::now();
		let bh = BuildHasherDefault::<FnvHasher>::default();
		let mut hasher = SuperMinHash2::<u64, String, _>::new(512, bh);
		for identifier in normalized_identifiers {
			hasher.sketch(&identifier).unwrap();
		}
		let minhash = hasher.get_hsketch();
		let hash_duration = hash_start_time.elapsed();

		let write_start_time = std::time::Instant::now();
		let mut writer = fs::OpenOptions::new()
		.append(true)
		.create(true)
		.open(&output)
		.unwrap();
		writer.write_all(format!("{},{},{},", project, index, i).as_bytes()).unwrap();
		writer.write_all(
			&minhash.iter()
			.map(|v| v.to_string())
			.collect::<Vec<String>>()
			.join(",")
			.as_bytes()
		).unwrap();
		writer.write_all(b"\n").unwrap();
		let write_duration = write_start_time.elapsed();

		println!("[{}][{}][{}] Filtered {} identifiers to {} symbols in {} ms, hashed in {} ms, wrote result in {} ms.", project, index, i, identifiers_count, symbol_count, filter_duration.as_millis(), hash_duration.as_millis(), write_duration.as_millis());
	}
}

struct HashData {
	project: String,
	index: String,
	chunk: String,
	hashes: Vec<u64>,
}

fn read_hash_data(
	file: &str
) -> Vec<HashData> {
	let file = File::open(file).expect("Failed to open hashes file");
	let reader = BufReader::new(file);
	let mut hash_data_list = Vec::new();
	for line in reader.lines() {
		let line = line.expect("Failed to read line");
		let tokens: Vec<&str> = line.split(',').collect();
		if tokens.len() < 4 {
			continue;
		}
		let project = tokens[0].to_string();
		let index = tokens[1].to_string();
		let chunk = tokens[2].to_string();
		let hashes: Vec<u64> = tokens[3..]
			.iter()
			.filter(|s| !s.is_empty())
			.map(|s| s.parse::<u64>().unwrap())
			.collect();
		hash_data_list.push(HashData {
			project,
			index,
			chunk,
			hashes,
		});
	}
	hash_data_list
}

pub fn find_most_similar(
	results_file: &str,
	name: &str,
) {
	let hash_data_list = read_hash_data(results_file);
	let from = hash_data_list.iter()
		.filter(|hd| hd.project == name);

	for item in from {
		let mut results = Vec::new();
		for hash_data in &hash_data_list {
			let similarity = get_jaccard_index_estimate(
				&hash_data.hashes,
				&item.hashes,
			).unwrap();
			results.push((similarity, hash_data));
		}

		results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

		for (i, (similarity, hash_data)) in results.iter().take(10).enumerate() {
			println!(
				"{}. {}[{}][{}] to {}[{}][{}] - Similarity: {:.4}",
				i + 1,
				item.project,
				item.index,
				item.chunk,
				hash_data.project,
				hash_data.index,
				hash_data.chunk,
				similarity
			);
		}
	}
}