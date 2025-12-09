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
	for lang_before in symbols_before.keys() {
		for lang_after in symbols_after.keys() {
			let names_before = &symbols_before[lang_before];
			let names_after = &symbols_after[lang_after];

			let intersection = names_before.intersection(&names_after).collect::<HashSet<_>>();

			let other_names_after = symbols_after.iter()
				.filter(|s| *s.0 != *lang_after)
				.flat_map(|s| s.1.iter())
				.collect::<HashSet<_>>();

			let moved = intersection.difference(&other_names_after).collect::<HashSet<_>>();

			let mut common_meansd = MeanSD::default();
			intersection.iter()
				.for_each(|s| { common_meansd.update(s.len() as f64); });
			let mut moved_meansd = MeanSD::default();
			moved.iter()
				.for_each(|s| { moved_meansd.update(s.len() as f64); });

			movement.insert(
				(*lang_before, *lang_after),
				SymbolMovement {
					common_count: intersection.len(),
					common_len_mean: common_meansd.mean(),
					common_len_stddev: common_meansd.sstdev(),
					moved_count: moved.len(),
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
	let buf_writer = std::io::BufWriter::new(file);
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
			pb.set_message(format!("from {}: {} analyzing identifiers", before_idx, repo_stats.name));
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