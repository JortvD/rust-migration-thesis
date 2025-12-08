use std::{
    collections::HashSet, fs::{self, File}, hash::BuildHasherDefault, io::{BufRead, BufReader, BufWriter, Read, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex}
};

use flate2::bufread::GzDecoder;
use fnv::FnvHasher;
use indicatif::ProgressBar;
use probminhash::superminhasher2::{get_jaccard_index_estimate, SuperMinHash2};

const MIN_LENGTH: usize = 0;
const NUM_HASHES: usize = 1024;

fn read_u32<R: Read>(reader: &mut R) -> std::io::Result<u32> {
    let mut buffer = [0u8; 4];
    reader.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

fn read_u64<R: Read>(reader: &mut R) -> std::io::Result<u64> {
    let mut buffer = [0u8; 8];
    reader.read_exact(&mut buffer)?;
    Ok(u64::from_le_bytes(buffer))
}

struct MatchOptions {
	include_languages: Option<HashSet<String>>,
	exclude_languages: Option<HashSet<String>>,
}

fn scan_repo_identifiers<F>(folder: &Path, mut callback: F, options: &MatchOptions)
where
    F: FnMut(&str),
{
    let paths = fs::read_dir(folder).expect("Failed to read directory");
    
    // reusable buffers to reduce allocation overhead
    let mut line_buffer = String::with_capacity(512);
    let mut norm_buffer = String::with_capacity(128);

    for path in paths {
        let path = path.expect("Failed to read path").path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("gz") {
            let file = File::open(&path).expect("Failed to open file");
            let reader = BufReader::with_capacity(64 * 1024, file);
            let mut decoder = BufReader::new(GzDecoder::new(reader));

            loop {
                line_buffer.clear();
                match decoder.read_line(&mut line_buffer) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
						let parts: Vec<&str> = line_buffer.split(',').collect();
						if parts.len() > 3 {
							let language_part = parts[0].trim();
							let name_part = parts[3].trim();

							// Language filtering
							if let Some(include_langs) = &options.include_languages {
								if !include_langs.contains(language_part) {
									continue;
								}
							}
							if let Some(exclude_langs) = &options.exclude_languages {
								if exclude_langs.contains(language_part) {
									continue;
								}
							}

							if name_part.len() >= MIN_LENGTH {
								norm_buffer.clear();
								// Normalization Logic
								for c in name_part.chars() {
									if c != '_' {
										norm_buffer.push(c.to_ascii_lowercase());
									}
								}
								
								// Pass normalized string to the callback
								callback(&norm_buffer);
							}
						}
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

pub fn get_identifiers_for_repo(folder: PathBuf, from: bool) -> HashSet<String> {
    let mut identifiers = HashSet::new();
    
    scan_repo_identifiers(&folder, |identifier| {
        identifiers.insert(identifier.to_string());
    }, &MatchOptions {
		include_languages: if from { Some(HashSet::from(["Rust".to_string()])) } else { None },
		exclude_languages: if from { None } else { Some(HashSet::from(["Rust".to_string()])) },
	});

    identifiers
}

pub fn hash_for_project(
    folder: PathBuf,
    output: &Arc<Mutex<BufWriter<File>>>,
    bar: &ProgressBar,
) {
    let project_name = folder.file_name().unwrap().to_str().unwrap().to_string();
    
    let bh = BuildHasherDefault::<FnvHasher>::default();
    let mut hasher = SuperMinHash2::<u64, String, _>::new(NUM_HASHES, bh);

    let mut total_filtered_count: u64 = 0;

    // Use the abstraction here
    scan_repo_identifiers(&folder, |identifier| {
        hasher.sketch(&identifier.to_string()).unwrap();
        total_filtered_count += 1;
    }, &MatchOptions {
		include_languages: None,
		exclude_languages: None,
	});
    
    bar.inc(1);
    bar.set_message(format!("{}: Processed identifiers", project_name));

    let minhash = hasher.get_hsketch();
    
    // ... rest of the writing logic ...
    let mut writer = output.lock().unwrap();
    let name_bytes = project_name.as_bytes();
    writer.write_all(&(name_bytes.len() as u32).to_le_bytes()).unwrap();
    writer.write_all(name_bytes).unwrap();
    writer.write_all(&total_filtered_count.to_le_bytes()).unwrap();
    writer.write_all(&(minhash.len() as u32).to_le_bytes()).unwrap();
    for hash in minhash {
        writer.write_all(&hash.to_le_bytes()).unwrap();
    }
}

#[derive(Debug)]
struct HashData {
    project: String,
    size: u64,
    hashes: Vec<u64>,
}

fn read_hash_data(file_path: &str) -> Vec<HashData> {
    let file = File::open(file_path).expect("Failed to open binary hashes file");
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut data = Vec::new();

    loop {
        let name_len = match read_u32(&mut reader) {
            Ok(n) => n,
            Err(_) => break,
        };

        let mut name_buffer = vec![0u8; name_len as usize];
        if reader.read_exact(&mut name_buffer).is_err() { break; }
        let project = String::from_utf8_lossy(&name_buffer).to_string();

        let size = match read_u64(&mut reader) {
            Ok(s) => s,
            Err(_) => break,
        };

        let hash_count = match read_u32(&mut reader) {
            Ok(c) => c,
            Err(_) => break,
        };

        let mut hashes = Vec::with_capacity(hash_count as usize);
        for _ in 0..hash_count {
             match read_u64(&mut reader) {
                Ok(h) => hashes.push(h),
                Err(_) => break,
            }
        }

        data.push(HashData {
            project,
            size,
            hashes,
        });
    }

    data
}

pub fn find_most_similar(
    results_file: &str,
	results_folder: &str,
    name: &str,
) {
    println!("Loading binary data...");
    let hash_data_list = read_hash_data(results_file);
    
    let from: Option<&HashData> = hash_data_list.iter()
        .find(|hd| hd.project == name);

    if from.is_none() {
        println!("Project {} not found in hash data.", name);
        return;
    }
    let item = from.unwrap();

    let mut results = Vec::with_capacity(hash_data_list.len());
    
    for hash_data in &hash_data_list {
        if hash_data.project == name { continue; }

        let similarity = get_jaccard_index_estimate(
            &hash_data.hashes,
            &item.hashes,
        ).unwrap_or(0.0);
        
        results.push((similarity, hash_data));
    }

    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    println!("Most similar to {} (processed {} items):", item.project, item.size);
	let from_identifiers = get_identifiers_for_repo(PathBuf::from(&format!("{}/{}", results_folder, item.project)), true);
    for (i, (similarity, hash_data)) in results.iter().take(50).enumerate() {
		let to_identifiers = get_identifiers_for_repo(PathBuf::from(&format!("{}/{}", results_folder, hash_data.project)), false);
		let common: HashSet<_> = from_identifiers.intersection(&to_identifiers).collect();
		let common_count = common.len();
		let union_count = from_identifiers.union(&to_identifiers).count();
		let jaccard_index = if union_count > 0 {
			common_count as f64 / union_count as f64
		} else {
			0.0
		};
		let percentage_from = if from_identifiers.len() > 0 {
			common_count as f64 / from_identifiers.len() as f64 * 100.0
		} else {
			0.0
		};
		let percentage_to = if to_identifiers.len() > 0 {
			common_count as f64 / to_identifiers.len() as f64 * 100.0
		} else {
			0.0
		};
        println!(
            "  {}. {} - Similarity: {:.4} (Size: {}) - Common Identifiers: {} (Jaccard Index: {:.4}, From: {:.2}%, To: {:.2}%)",
            i + 1,
            hash_data.project,
            similarity,
            hash_data.size,
			common_count,
			jaccard_index,
			percentage_from,
			percentage_to
        );
		let mut common: Vec<_> = common.into_iter().collect();
		common.sort_by(|a, b| b.len().cmp(&a.len()));
        for common_identifier in common.iter().take(5) {
			println!("    Common Identifier: {}", common_identifier);
		}
    }
}