use tokei::LanguageType;
use std::collections::{HashMap, HashSet};

use crate::{code, consts::DEBUG, gather::RepositoryStats, math};

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

const MIN_SYMBOL_NAME_LENGTH: usize = 4;

fn symbol_vec_to_names_set(
	symbols: &Vec<code::Symbol>,
) -> HashSet<&String> {
	symbols.iter().map(|s| &s.name).filter(|name| !name.is_empty() && name.len() >= MIN_SYMBOL_NAME_LENGTH).collect()
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

			let names_before: HashSet<&String> = symbol_vec_to_names_set(lang_symbols_before);
			let names_after: HashSet<&String> = symbol_vec_to_names_set(lang_symbols_after);

			let intersection = names_before.intersection(&names_after).collect::<HashSet<_>>();

			let common_count = intersection.len();

			let other_symbols_after = symbols_after.iter()
				.filter(|s| *s.0 != *lang_after)
				.flat_map(|s| symbol_vec_to_names_set(s.1).into_iter())
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
					before_count: names_before.len(),
					after_count: names_after.len(),
				},
			);
		}
	}

	SymbolAnalysis {
		movement,
	}
}

#[derive(Debug)]
pub enum RustAdditionResult {
	NeverAdded,
	AddedAboveThreshold(f64, usize),
	AlwaysPresent,
	NoStats,
}

fn test_rust_was_added(
	writer: &mut dyn std::io::Write,
	repo_stats: &RepositoryStats,
	threshold: f64,
) -> RustAdditionResult {
	if let Some(first_stats) = repo_stats.lang_stats.first() {
		let initial_rust_pct = first_stats.get(&LanguageType::Rust).map_or(0.0, |s| s.0);
		if initial_rust_pct == 0.0 {
			for (i, stats) in repo_stats.lang_stats.iter().enumerate().skip(1) {
				let rust_pct = stats.get(&LanguageType::Rust).map_or(0.0, |s| s.0);
				if rust_pct > threshold {
					writeln!(
						writer,
						"Detected first signficant amount of Rust ({:.2}%) at point {}",
						rust_pct * 100.0,
						i
					).expect("Failed to write to writer");

					return RustAdditionResult::AddedAboveThreshold(rust_pct, i);
				}
			}

			writeln!(
				writer,
				"No Rust added above threshold ({:.2}%) in analyzed history.",
				threshold * 100.0
			).expect("Failed to write to writer");
			RustAdditionResult::NeverAdded
		} else {
			writeln!(
				writer,
				"Rust was already present at the start of the analyzed history ({:.2}%)",
				initial_rust_pct * 100.0
			).expect("Failed to write to writer");
			RustAdditionResult::AlwaysPresent
		}
	} else {
		writeln!(
			writer,
			"No language statistics available to analyze Rust addition."
		).expect("Failed to write to writer");

		RustAdditionResult::NoStats
	}
}

#[derive(Debug)]
pub enum CodeMovementResult {
	MovementInProgress(code::SupportedLanguage, usize, f64),
	SignificantMovement(code::SupportedLanguage, usize, f64),
	SignificantCommon(code::SupportedLanguage, usize, f64),
	NoSignificantMovement,
	NoMigrationPoint,
}

fn test_code_moved_to_rust(
	writer: &mut dyn std::io::Write,
	repo_stats: &RepositoryStats,
	min_moved_count: usize,
	moved_threshold: f64,
	min_common_count: usize,
	common_threshold: f64,
	min_slope: f64,
) -> CodeMovementResult {
	let migration_idx = match find_migration_start_index(repo_stats) {
		Some(idx) => idx,
		None => return CodeMovementResult::NoMigrationPoint,
	};
	
	if migration_idx == 0 || migration_idx >= repo_stats.symbols.len() {
		return CodeMovementResult::NoMigrationPoint;
	}

	let mut max_common = 0;
	let mut max_common_pct = 0.0;
	let mut max_common_lang = None;

	let length = repo_stats.symbols.len();

	// NOTE: Only take 3 because of performance concerns
	for x in ((migration_idx-1)..(length-1)).take(3) {
		let mut max_moved = 0;
		let mut max_moved_pct = 0.0;
		let mut max_moved_lang = None;
		
		let mut moved_map = HashMap::new();
		let symbols_before = &repo_stats.symbols[x];

		for (i, symbols_after) in repo_stats.symbols[(x+1)..].iter().enumerate() {
			let analysis = analyze_symbols(symbols_before, symbols_after);
			for ((lang_before, lang_after), movement) in analysis.movement.iter() {
				if *lang_after == code::SupportedLanguage::Rust && *lang_before != code::SupportedLanguage::Rust {
					if max_moved < movement.moved_count {
						max_moved = movement.moved_count;
						max_moved_lang = Some(lang_before.clone());
						max_moved_pct = movement.moved_percentage;
					}
					if max_common < movement.common_count {
						max_common = movement.common_count;
						max_common_lang = Some(lang_before.clone());
						max_common_pct = movement.common_count as f64 / movement.after_count as f64;
					}
					
					moved_map.entry(lang_before.clone()).or_insert_with(Vec::new).push(movement.moved_count);
					
					writeln!(
						writer,
						"[{}->{}] {:?} to {:?}: moved {} / {} common symbols, or {:.2}% ({:?} total {}, {:?} total {})", 
						x,
						i + x + 1,
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

					if movement.moved_percentage >= moved_threshold && movement.moved_count >= min_moved_count {	
						return CodeMovementResult::SignificantMovement(lang_before.clone(), movement.moved_count, movement.moved_percentage);
					}
				}
			}
		}

		writeln!(
			writer,
			"[{}] Max moved symbols is too low for {:?}: {} < {} or {} < {:.2}%",
			x,
			max_moved_lang,
			max_moved,
			min_moved_count,
			max_moved_pct * 100.0,
			moved_threshold * 100.0
		).expect("Failed to write to writer");

		if let Some(lang) = max_moved_lang {
			let moved_counts = &moved_map[&lang];
			if let Some(slope) = math::least_squares_slope(moved_counts) {
				if slope > min_slope && max_moved >= min_moved_count {
					writeln!(
						writer,
						"[{}] Detected increasing trend for {:?} with slope {:.4}",
						x, lang, slope
					)
					.expect("Failed to write to writer");

					return CodeMovementResult::MovementInProgress(lang.clone(), max_moved, slope);
				} else {
					writeln!(
						writer,
						"[{}] No significant increasing trend for {:?} (slope {:.4} < {:.4} or max_moved {} < {})",
						x, lang, slope, min_slope, max_moved, min_moved_count
					)
					.expect("Failed to write to writer");
				}
			}
		}
	}

	if let Some(lang) = max_common_lang {
		if max_common >= min_common_count && max_common_pct >= common_threshold {
			writeln!(
				writer,
				"There were {} common symbols from {:?} ({:.2}%), indicating some level of migration or duplication.",
				max_common,
				lang,
				max_common_pct * 100.0
			).expect("Failed to write to writer");

			return CodeMovementResult::SignificantCommon(lang, max_common, max_common_pct);
		} else {
			writeln!(
				writer,
				"Max common is too low for {:?}: {} < {} symbols or {:.2}% < {:.2}%",
				lang,
				max_common,
				min_common_count,
				max_common_pct * 100.0,
				common_threshold * 100.0
			).expect("Failed to write to writer");
		}
	}

	CodeMovementResult::NoSignificantMovement
}

pub fn check_migration_status(
	mut repo_stats: RepositoryStats,
	writer: &mut dyn std::io::Write,
) -> (RustAdditionResult, CodeMovementResult) {
	let rust_added = test_rust_was_added(writer, &repo_stats, 0.01);
	let code_moved = test_code_moved_to_rust(writer, &repo_stats, 50, 0.75, 100, 0.10, 1.0);

	repo_stats.symbols.clear();
	repo_stats.symbols.shrink_to_fit();
	repo_stats.lang_stats.clear();
	repo_stats.lang_stats.shrink_to_fit();
	(rust_added, code_moved)
}