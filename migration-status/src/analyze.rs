use tokei::LanguageType;
use std::collections::{HashMap, HashSet};

use crate::{code, consts::DEBUG, gather::RepositoryStats};

fn find_migration_start_index(
	repo_stats: &RepositoryStats,
) -> Option<usize> {
	for (i, lang_stat) in repo_stats.lang_stats.iter().enumerate() {
		let rust_pct = lang_stat.get(&LanguageType::Rust).map_or(0.0, |s| s.0);
		
		if rust_pct > 0.0 {
			return Some(i);
		}
	}
	None
}

struct SymbolMovement {
	common_count: usize,
	moved_count: usize,
	moved_percentage: f64,
	before_count: usize,
	after_count: usize,
}

struct SymbolAnalysis {
	movement: HashMap<(code::SupportedLanguage, code::SupportedLanguage), SymbolMovement>,
}

fn analyze_symbols (
	symbols_before: &HashMap<code::SupportedLanguage, Vec<code::Symbol>>,
	symbols_after: &HashMap<code::SupportedLanguage, Vec<code::Symbol>>,
) -> SymbolAnalysis {
	let mut movement = HashMap::new();
	for lang_before in symbols_before.keys() {
		for lang_after in symbols_after.keys() {
			let lang_symbols_before = &symbols_before[lang_before];
			let lang_symbols_after = &symbols_after[lang_after];

			let names_before: HashSet<&String> = lang_symbols_before.iter().map(|s| &s.name).collect();
			let names_after: HashSet<&String> = lang_symbols_after.iter().map(|s| &s.name).collect();

			let intersection = names_before.intersection(&names_after).collect::<HashSet<_>>();

			let common_count = intersection.len();

			let other_symbols_after = symbols_after.iter()
				.filter(|s| *s.0 != *lang_after)
				.flat_map(|s| s.1.iter().map(|f| &f.name))
				.collect::<HashSet<_>>();

			let moved_count = intersection.difference(&other_symbols_after.iter().collect::<HashSet<_>>()).count();

			movement.insert(
				(*lang_before, *lang_after),
				SymbolMovement {
					common_count,
					moved_count,
					moved_percentage: if common_count > 0 {
						moved_count as f64 / common_count as f64
					} else {
						0.0
					},
					before_count: lang_symbols_before.len(),
					after_count: lang_symbols_after.len(),
				},
			);
		}
	}

	SymbolAnalysis {
		movement,
	}
}

fn test_rust_was_added(
	writer: &mut dyn std::io::Write,
	repo_stats: &RepositoryStats,
	threshold: f64,
) -> bool {
	if let Some(first_stats) = repo_stats.lang_stats.first() {
		let initial_rust_pct = first_stats.get(&LanguageType::Rust).map_or(0.0, |s| s.0);
		if initial_rust_pct == 0.0 {
			for (i, stats) in repo_stats.lang_stats.iter().enumerate().skip(1) {
				let rust_pct = stats.get(&LanguageType::Rust).map_or(0.0, |s| s.0);
				if rust_pct > threshold {
					writeln!(
						writer,
						"Detected first Rust ({:.2}%) in tree at commit {}",
						rust_pct * 100.0,
						i
					).expect("Failed to write to writer");
					return true;
				}
			}
		}
	}
	false
}

fn test_code_moved_to_rust(
	writer: &mut dyn std::io::Write,
	repo_stats: &RepositoryStats,
	min_count: usize,
	threshold: f64,
) -> bool {
	let migration_idx = match find_migration_start_index(repo_stats) {
		Some(idx) => idx,
		None => return false,
	};
	if migration_idx == 0 || migration_idx >= repo_stats.symbols.len() {
		return false;
	}
	let mut max_moved = 0;
	let mut max_moved_language = None;
	let symbols_before = &repo_stats.symbols[migration_idx-1];
	for (i, symbols_after) in repo_stats.symbols[migration_idx..].iter().enumerate() {
		let start_time = std::time::Instant::now();
		let analysis = analyze_symbols(symbols_before, symbols_after);
		for ((lang_before, lang_after), movement) in analysis.movement.iter() {
			if *lang_after == code::SupportedLanguage::Rust && *lang_before != code::SupportedLanguage::Rust {
				if max_moved < movement.moved_count {
					max_moved = movement.moved_count;
					max_moved_language = Some(lang_before.clone());
				}
				
				writeln!(
					writer,
					"[{}] {:?} to {:?}: moved {} / {} common symbols, or {:.2}% ({:?} total {}, {:?} total {})", 
					i + migration_idx,
					lang_before,
					lang_after,
					movement.moved_count,
					movement.common_count,
					movement.moved_percentage * 100.0,
					lang_before,
					movement.before_count,
					lang_after,
					movement.after_count,
				).expect("Failed to write to writer");

				if movement.moved_percentage >= threshold && movement.moved_count >= min_count {	
					return true;
				}
			}
			let elapsed = start_time.elapsed();
			writeln!(
				writer,
				"[{}] Analysis took {:.2?}",
				i + migration_idx,
				elapsed
			).expect("Failed to write to writer");
		}
	}
	writeln!(
		writer,
		"No significant symbol movement to Rust detected. Maximum was {} symbols from {:?}",
		max_moved,
		max_moved_language
	).expect("Failed to write to writer");
	false
}

pub enum MigrationStatus {
	Migration,
	NoMigration,
}

pub fn check_migration_status(
	repo_stats: &RepositoryStats,
	writer: &mut dyn std::io::Write,
) -> MigrationStatus {
	let rust_added = test_rust_was_added(writer, repo_stats, 0.01);
	let code_moved = test_code_moved_to_rust(writer, repo_stats, 50, 0.75);

	if rust_added && code_moved {
		MigrationStatus::Migration
	} else {
		MigrationStatus::NoMigration
	}
}