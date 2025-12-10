use std::{collections::{HashMap, HashSet}, fs, io::BufRead};
use std::io::Write;
use indicatif::ProgressBar;
use meansd::MeanSD;

use crate::{code, gather::RepositoryStats};

struct SymbolMovement {
	moved_count: usize,
	moved_len_mean: f64,
	moved_len_stddev: f64,
	common_count: usize,
	common_len_mean: f64,
	common_len_stddev: f64,
	before_count: usize,
	after_count: usize,
}

struct SymbolAnalysis {
	movement: HashMap<(code::SupportedLanguage, code::SupportedLanguage), SymbolMovement>,
}

fn analyze_symbols (
	symbols_before: &HashMap<code::SupportedLanguage, HashSet<String>>,
	symbols_after: &HashMap<code::SupportedLanguage, HashSet<String>>,
) -> SymbolAnalysis {

	let mut movement = HashMap::new();

	let mut after_symbol_counts: HashMap<&String, usize> = HashMap::new();
    for names in symbols_after.values() {
        for name in names {
            *after_symbol_counts.entry(name).or_insert(0) += 1;
        }
    }

	for (lang_before, names_before) in symbols_before {
		for (lang_after, names_after) in symbols_after {
			let mut common_meansd = MeanSD::default();
            let mut moved_meansd = MeanSD::default();
            let mut common_count = 0;
            let mut moved_count = 0;

			for symbol in names_before {
                if names_after.contains(symbol) {
                    common_count += 1;
                    common_meansd.update(symbol.len() as f64);

                    if let Some(&count) = after_symbol_counts.get(symbol) {
                        if count == 1 {
                            moved_count += 1;
                            moved_meansd.update(symbol.len() as f64);
                        }
                    }
                }
            }

			movement.insert(
				(*lang_before, *lang_after),
				SymbolMovement {
					common_count,
					common_len_mean: common_meansd.mean(),
					common_len_stddev: common_meansd.sstdev(),
					moved_count,
					moved_len_mean: moved_meansd.mean(),
					moved_len_stddev: moved_meansd.sstdev(),
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

fn get_results_file_writer(results_folder: &str, name: &str) -> flate2::write::GzEncoder<std::io::BufWriter<std::fs::File>> {
	let file = fs::OpenOptions::new()
		.create(true)
		.read(true)
		.append(true)
		.open(format!("{}/{}.csv.gz", results_folder, name))
		.expect("Failed to open language presence file");
	let buf_writer = std::io::BufWriter::with_capacity(64 * 1024, file);
	flate2::write::GzEncoder::new(buf_writer, flate2::Compression::default())
}

fn language_presence_analysis(repo_stats: &RepositoryStats) {
	let mut writer = get_results_file_writer(&repo_stats.results_folder, "languages");
	for (i, lang_stat) in repo_stats.lang_stats.iter().enumerate() {
		for (lang, (pct, loc, blanks, comments)) in lang_stat {
			writeln!(
				writer,
				"{},{},{},{},{},{}",
				i,
				lang.to_string(),
				pct,
				loc,
				blanks,
				comments
			).expect("Failed to write to language presence file");
		}
	}
	writer.finish().expect("Failed to finish writing language presence file");
}

fn get_symbols(
    results_folder: &str,
    index: usize,
    languages: &HashSet<code::SupportedLanguage>,
) -> HashMap<code::SupportedLanguage, HashSet<String>> {
    let mut symbols_map = HashMap::new();
    for lang in languages {
        let file_path = format!("{}/{}_{}_symbols.txt", results_folder, index, lang.to_string());
        
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            let mut symbols_set = HashSet::new();
            for line in content.lines() {
                symbols_set.insert(line.to_string());
            }
            symbols_map.insert(*lang, symbols_set);
        }
    }
    symbols_map
}

fn identifier_analysis(
	repo_stats: &RepositoryStats,
	pb: &ProgressBar,
) {
	let mut writer = get_results_file_writer(&repo_stats.results_folder, "identifiers");
	let length = repo_stats.symbols.len();

	for before_idx in 0..(length - 1) {
		let symbols_before = get_symbols(
			&repo_stats.results_folder,
			before_idx,
			&repo_stats.symbols[before_idx].iter().cloned().collect()
		);
		for after_idx in (before_idx + 1)..length {
			pb.set_message(format!("{}->{}: {} analyzing identifiers", before_idx, after_idx, repo_stats.name));
			let symbols_after = get_symbols(
				&repo_stats.results_folder,
				after_idx,
				&repo_stats.symbols[after_idx].iter().cloned().collect()
			);
			let analysis = analyze_symbols(&symbols_before, &symbols_after);
			for ((lang_before, lang_after), movement) in analysis.movement.iter() {
				writeln!(
					writer,
					"{},{},{},{},{},{},{},{},{},{},{},{}",
					before_idx,
					after_idx,
					lang_before.to_string(),
					lang_after.to_string(),
					movement.moved_count,
					movement.moved_len_mean,
					movement.moved_len_stddev,
					movement.common_count,
					movement.common_len_mean,
					movement.common_len_stddev,
					movement.before_count,
					movement.after_count,
				).expect("Failed to write to identifier movement file");
			}
		}
	}
	writer.finish().expect("Failed to finish writing identifier movement file");
}

pub fn run_analysis(repo_stats: RepositoryStats, pb: &ProgressBar) {
	pb.set_message(format!("{} Analyzing languages", repo_stats.name));
	language_presence_analysis(&repo_stats);
	identifier_analysis(&repo_stats, pb);
}