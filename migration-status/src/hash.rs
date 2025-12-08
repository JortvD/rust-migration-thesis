use std::{
    fs::{self, File},
    hash::BuildHasherDefault,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use flate2::bufread::GzDecoder;
use fnv::FnvHasher;
use indicatif::ProgressBar;
use probminhash::superminhasher2::{get_jaccard_index_estimate, SuperMinHash2};

const MIN_LENGTH: usize = 10;
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

pub fn hash_for_project(
    folder: PathBuf,
    output: &Arc<Mutex<BufWriter<File>>>,
    bar: &ProgressBar,
) {
    let project_name = folder.file_name().unwrap().to_str().unwrap().to_string();
    
    let bh = BuildHasherDefault::<FnvHasher>::default();
    let mut hasher = SuperMinHash2::<u64, String, _>::new(NUM_HASHES, bh);

    let mut total_filtered_count: u64 = 0;
    
    let mut line_buffer = String::with_capacity(512);
    let mut norm_buffer = String::with_capacity(128);

    let paths = fs::read_dir(&folder).expect("Failed to read directory");

    for path in paths {
        let path = path.expect("Failed to read path").path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("gz") {
            let file = File::open(&path).expect("Failed to open file");
            let reader = BufReader::with_capacity(64 * 1024, file); 
            let mut decoder = BufReader::new(GzDecoder::new(reader));

            loop {
                line_buffer.clear();
                match decoder.read_line(&mut line_buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        let mut commas_found = 0;
                        let mut start_idx = 0;
                        let mut end_idx = 0;
                        
                        for (i, b) in line_buffer.bytes().enumerate() {
                            if b == b',' {
                                commas_found += 1;
                                if commas_found == 3 {
                                    start_idx = i + 1;
                                } else if commas_found == 4 {
                                    end_idx = i;
                                    break;
                                }
                            }
                        }

                        if commas_found >= 3 {
                            if commas_found == 3 { end_idx = line_buffer.trim_end().len(); }
                            
                            if start_idx < end_idx {
                                let name_part = &line_buffer[start_idx..end_idx];
                                
                                if name_part.len() >= MIN_LENGTH {
                                    norm_buffer.clear();
                                    for c in name_part.chars() {
                                        if c != '_' {
                                            norm_buffer.push(c.to_ascii_lowercase());
                                        }
                                    }

                                    hasher.sketch(&norm_buffer).unwrap();
                                    total_filtered_count += 1;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
    
    bar.inc(1);
    bar.set_message(format!("{}: Processed identifiers", project_name));

    let minhash = hasher.get_hsketch();
    
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
    for (i, (similarity, hash_data)) in results.iter().take(50).enumerate() {
        println!(
            "  {}. {} - Similarity: {:.4} (Size: {})",
            i + 1,
            hash_data.project,
            similarity,
            hash_data.size
        );
    }
}