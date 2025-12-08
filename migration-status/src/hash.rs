use std::{collections::HashSet, fs::{self, File}, hash::BuildHasherDefault, io::{BufRead, BufReader, Read, Write}, path::PathBuf};

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

fn process_file_identifiers(file_path: &PathBuf) -> (usize, usize, Vec<String>) {
    let file = File::open(file_path).expect("Failed to open file");
    let reader = BufReader::new(file);
    let decoder = BufReader::new(GzDecoder::new(reader)); 

    let mut total_count = 0;
    let mut filtered_count = 0;
    let mut unique_identifiers = std::collections::HashSet::new();

    for line in decoder.lines().filter_map(|line| line.ok()) {
        if let Some(identifier) = parse_identifier(&line) {
            total_count += 1;
            if identifier.name.len() >= MIN_LENGTH {
                filtered_count += 1;
                let normalized: String = identifier.name.chars()
                    .filter(|c| *c != '_')
                    .map(|c| c.to_ascii_lowercase())
                    .collect();
                unique_identifiers.insert(normalized);
            }
        }
    }

    (total_count, filtered_count, unique_identifiers.into_iter().collect())
}

const MIN_LENGTH: usize = 5;

pub fn hash_for_project(
    folder: PathBuf,
    output: String
) {
    let project = folder.file_name().unwrap().to_str().unwrap().to_string();
	let start_time = std::time::Instant::now();

    let bh = BuildHasherDefault::<FnvHasher>::default();
    let mut hasher = SuperMinHash2::<u64, String, _>::new(1024, bh);
    
    let mut total_identifiers_count = 0;
    let mut total_filtered_count = 0;
    let mut total_symbol_count = 0;

    let paths = fs::read_dir(&folder).expect("Failed to read directory");

    for path in paths {
        let path = path.expect("Failed to read path").path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("gz") {
            let (file_count, file_filtered_count, normalized_identifiers) = process_file_identifiers(&path);
            
            total_identifiers_count += file_count;
            total_filtered_count += file_filtered_count;
            total_symbol_count += normalized_identifiers.len();

            for identifier in normalized_identifiers {
                hasher.sketch(&identifier).unwrap();
            }
        }
    }
    
    let minhash = hasher.get_hsketch();
    
    let mut writer = fs::OpenOptions::new()
    .append(true)
    .create(true)
    .open(&output)
    .unwrap();
    writer.write_all(format!("{},{},", project, 0).as_bytes()).unwrap();
    writer.write_all(
        &minhash.iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join(",")
        .as_bytes()
    ).unwrap();
    writer.write_all(b"\n").unwrap();

    // println!(
	// 	"[{}] {} -> {} -> {} symbols (total) processed and hashed in {} ms.", 
	// 	project, 
	// 	total_identifiers_count, 
	// 	total_filtered_count, 
	// 	total_symbol_count,
	// 	start_time.elapsed().as_millis()
	// );
}

struct HashData {
	project: String,
	size: u64,
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
		if tokens.len() < 3 {
			continue;
		}
		let project = tokens[0].to_string();
		let size = tokens[1].parse::<u64>().unwrap_or(0);
		let hashes: Vec<u64> = tokens[2..]
			.iter()
			.filter(|s| !s.is_empty())
			.map(|s| s.parse::<u64>().unwrap())
			.collect();
		hash_data_list.push(HashData {
			project,
			size,
			hashes,
		});
	}
	hash_data_list
}

// fn compare_identifiers(
// 	from: &str,
// 	to: &str,
// ) -> f64 {
// 	let from_identifiers = read_identifiers_in_folder(from);
// 	let to_identifiers = read_identifiers_in_folder(to);

// 	let from_set: HashSet<String> = normalize_and_deduplicate_identifiers(filter_identifiers(from_identifiers)).into_iter().collect();
// 	let to_set: HashSet<String> = normalize_and_deduplicate_identifiers(filter_identifiers(to_identifiers)).into_iter().collect();

// 	let intersection: usize = from_set.intersection(&to_set).count();
// 	let union: usize = from_set.union(&to_set).count();

// 	let jaccard_index = if union == 0 {
// 		0.0
// 	} else {
// 		intersection as f64 / union as f64
// 	};

// 	jaccard_index
// }

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
		println!("Most similar to {} (with {} symbols):", item.project, item.size);
		for (i, (similarity, hash_data)) in results.iter().take(10).enumerate() {
			// let exact_similarity = compare_identifiers(
			// 	&format!("results/symbols/{}", item.project),
			// 	&format!("results/symbols/{}", hash_data.project),
			// );
			println!(
				"  {}. {} - Hash similarity: {:.4} from {} symbols.",
				i + 1,
				hash_data.project,
				similarity,
				hash_data.size
			);
		}
	}
}