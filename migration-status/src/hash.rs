use std::{
    collections::{HashMap, HashSet}, fs::{self, File}, hash::BuildHasherDefault, io::{BufRead, BufReader, BufWriter, Read, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex}
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

const LANGUAGES: [&str; 24] = [
    "Rust", "C", "C++", "C#", "JavaScript", "TypeScript", "TSX", "Python", "Go", "Java", "Swift", "Dart", "Elixir", "Haskell", "Lua", "OCaml", "Ruby", "PHP", "Nix", "Bash", "Scala", "Scala", "Objective-C", "Clojure"
];

fn scan_repo_identifiers<F>(folder: &Path, mut callback: F)
where
    F: FnMut(&str, &str),
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
                        if parts.len() < 4 {
                            continue;
                        }
                        let language_part = parts[0].trim();

                        if !LANGUAGES.contains(&language_part) {
                            continue;
                        }
                        let name_part = if parts.len() < 5 {
                            // Identifier name contains an enter
                            let mut buffer2: String = String::new();
                            match decoder.read_line(&mut buffer2) {
                                Ok(0) => break, // EOF
                                Ok(_) => buffer2.trim(),
                                Err(_) => break,
                            };
                            let next_line_parts: Vec<&str> = buffer2.split(',').collect();
                            if next_line_parts.len() < 1 {
                                continue;
                            }
                            format!("{}{}", parts[3].trim(), next_line_parts[0].trim())
                        }
						else {
							parts[3].trim().to_string()
						};
                        if name_part.len() >= MIN_LENGTH {
                            norm_buffer.clear();
                            for c in name_part.chars() {
                                if c != '_' && c != '\n' && c != '\r' {
                                    norm_buffer.push(c.to_ascii_lowercase());
                                }
                            }
                            
                            callback(&norm_buffer, language_part);
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

pub fn get_identifiers_for_repo(folder: PathBuf) -> HashSet<String> {
    let mut identifiers = HashSet::new();
    
    scan_repo_identifiers(&folder, |identifier, _| {
        identifiers.insert(identifier.to_string());
    });

    identifiers
}

pub fn hash_for_project(
    folder: PathBuf,
    output: &Arc<Mutex<BufWriter<File>>>,
    bar: &ProgressBar,
) {
    let project_name = folder.file_name().unwrap().to_str().unwrap().to_string();
    bar.set_message(format!("{}: Processing", project_name));

    let bh = BuildHasherDefault::<FnvHasher>::default();
    let mut hasher_map = HashMap::new();

    scan_repo_identifiers(&folder, |identifier, language| {
        if !identifier.chars().all(|c| c.is_ascii_alphanumeric()) {
            return;
        }
        let entry = hasher_map.entry(language.to_string()).or_insert_with(|| {
            SuperMinHash2::<u64, String, _>::new(NUM_HASHES, bh.clone())
        });
        entry.sketch(&identifier.to_string()).unwrap();
    });
    
    bar.set_message(format!("{}: Writing for {} languages", project_name, hasher_map.len()));
    bar.inc(1);

    let mut writer = output.lock().unwrap();

    for (language, hasher) in hasher_map.iter() {
        let minhash = hasher.get_hsketch();
    
        let name_bytes = project_name.as_bytes();
        writer.write_all(&(name_bytes.len() as u32).to_le_bytes()).unwrap();
        writer.write_all(name_bytes).unwrap();
        let lang_bytes = language.as_bytes();
        writer.write_all(&(lang_bytes.len() as u32).to_le_bytes()).unwrap();
        writer.write_all(lang_bytes).unwrap();
        writer.write_all(&(minhash.len() as u32).to_le_bytes()).unwrap();
        for hash in minhash {
            writer.write_all(&hash.to_le_bytes()).unwrap();
        }
    }
}

#[derive(Debug)]
pub struct HashData {
    pub project: String,
    pub language: String,
    pub hashes: Vec<u64>,
}

pub fn read_hash_data(file_path: &str) -> Vec<HashData> {
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

        let lang_len = match read_u32(&mut reader) {
            Ok(n) => n,
            Err(_) => break,
        };
        let mut lang_buffer = vec![0u8; lang_len as usize];
        if reader.read_exact(&mut lang_buffer).is_err() { break; }
        let language = String::from_utf8_lossy(&lang_buffer).to_string();

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
            language,
            hashes,
        });
    }

    data
}

pub fn find_most_similar(
    results_file: &str,
	results_folder: &str,
    name: &str,
    langauge: &str,
) {
    let start_time = std::time::Instant::now();
    let hash_data_list = read_hash_data(results_file);
    println!(
        "Loaded {} hash data entries in {:.2?}",
        hash_data_list.len(),
        start_time.elapsed()
    );
    
    let from: Option<&HashData> = hash_data_list.iter()
        .find(|hd| hd.project == name && hd.language == langauge);

    if from.is_none() {
        println!("Project {} not found in hash data.", name);
        return;
    }
    let item = from.unwrap();
    let start_time = std::time::Instant::now();
    let mut results = Vec::with_capacity(hash_data_list.len());
    
    for hash_data in &hash_data_list {
        if hash_data.project == name { continue; }
        if hash_data.language == langauge { continue; }

        let similarity = get_jaccard_index_estimate(
            &hash_data.hashes,
            &item.hashes,
        ).unwrap_or(0.0);
        
        results.push((similarity, hash_data));
    }

    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    println!("Most similar projects to {} (language: {}, computed in {} ms):", name, langauge, start_time.elapsed().as_millis());
    for (i, (similarity, hash_data)) in results.iter().take(15).enumerate() {
         println!(
            "  {}. {} for {} - Similarity: {:.4}",
            i + 1,
            hash_data.project,
            hash_data.language,
            similarity,
        );
    }
}