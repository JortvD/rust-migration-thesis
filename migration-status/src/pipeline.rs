use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use indicatif::{ProgressBar, ProgressStyle, MultiProgress};

use rayon::prelude::*;

use crate::analyze;
use crate::code;
use crate::code::SupportedLanguage;
use crate::code::SymbolData;
use crate::gather;
use crate::hash;

fn filter_unique_repos(
	repos: Vec<(String, u32, f64)>,
) -> Vec<(String, u32, f64)> {
	let mut seen = HashSet::new();
	let mut unique_repos = Vec::new();

	for repo in repos {
		if !seen.contains(&repo.0) {
			seen.insert(repo.0.clone());
			unique_repos.push(repo);
		}
	}

	unique_repos
}

fn filter_min_rust(
	repos: Vec<(String, u32, f64)>,
	min_rust_pct: f64,
) -> Vec<(String, u32, f64)> {
	repos.into_iter()
		.filter(|repo| repo.2 >= min_rust_pct)
		.collect()
}

fn collect_repos(
	rdr: &mut csv::Reader<std::fs::File>,
) -> Vec<(String, u32, f64)> {
	let mut repos = Vec::new();

	for result in rdr.records() {
		let record = result.expect("Failed to read record");
		let full_name = record[1].to_string();
		let stars: u32 = record[2].parse().unwrap_or(0);
		let rust_percentage: f64 = record[3].parse().unwrap_or(0.0);

		repos.push((full_name, stars, rust_percentage));
	}

	repos
}

pub fn clean_temp_dir(
	temp_dir: &str,
) {
	let path = Path::new(temp_dir);
	if path.exists() {
		fs::remove_dir_all(path).expect("Failed to remove temp directory");
	}
}

pub fn clean_symbols(
	results_folder: &str,
) {
	let path = Path::new(results_folder);
	if path.exists() {
		let entries = fs::read_dir(path).expect("Failed to read results directory");

		for entry in entries {
			let entry = entry.expect("Failed to read directory entry");
			let file_name = entry.file_name();
			let file_name_str = file_name.to_string_lossy();

			if file_name_str.ends_with("symbols.txt") {
				fs::remove_file(entry.path()).expect("Failed to remove symbols file");
			}
		}
	}
}

pub fn run_analysis_for_repo(
	repo_full_name: &str,
	output_dir: &str,
	bar: &ProgressBar,
) {
	let parts: Vec<&str> = repo_full_name.split('/').collect();
	if parts.len() != 2 {
		bar.set_message(format!("{}: Invalid repository name, skipping", repo_full_name));
		return;
	}

	let owner = parts[0];
	let repo = parts[1];
	let result_folder = format!("{}/{}_{}", output_dir, owner, repo);

	if !Path::new(&result_folder).exists() {
		fs::create_dir_all(&result_folder).expect("Failed to create output directory");
	}

	let result_file = format!("{}/identifiers.csv.gz", result_folder);

	if Path::new(&result_file).exists() {
		bar.set_message(format!("{}: Output already exists, skipping", repo_full_name));
		return;
	}

	let temp_dir = format!("temp/{}_{}", owner, repo);

	let gather_result = gather::gather_repository_statistics(
		owner,
		repo,
		&result_folder,
		&temp_dir,
		100,
		bar
	);
	
	match gather_result {
		Ok(stats) => {
			analyze::run_analysis(stats, bar);
			bar.set_message(format!("{}: Completed analysis", repo_full_name));
		}
		Err(e) => {
			bar.set_message(format!("{}: Error during gathering: {:?}", repo_full_name, e));
		}
	}
	clean_temp_dir(&temp_dir);
	clean_symbols(&result_folder);
}

pub fn run_analysis_pipeline(
	input_csv: &str,
	output_dir: &str,
) {
	let mut rdr = csv::Reader::from_path(input_csv).expect("Failed to open CSV file");
	let mut repos = collect_repos(&mut rdr);
	println!("Collected {} repositories from CSV.", repos.len());
	repos = filter_unique_repos(repos);
	println!("Filtered to {} unique repositories.", repos.len());
	repos = filter_min_rust(repos, 1.0);
	println!("Filtered to {} repositories with at least 1.0% Rust.", repos.len());
	repos.sort_by(|a, b| b.1.cmp(&a.1));

	// Take only 10 for testing
	// repos = repos.iter().take(10).cloned().collect();

	let total_repos = repos.len();
	let mp = MultiProgress::new();
	let overall_bar = mp.add(ProgressBar::new(total_repos as u64));
    overall_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "[{elapsed_precise}] [{bar:100.cyan/blue}] {pos}/{len} ({eta})"
            )
            .expect("Failed to create template")
            .progress_chars("#>-"),
    );

	let mut handles: HashMap<usize, ProgressBar> = HashMap::new();
	for thread in 0..rayon::current_num_threads() {
		let pb = mp.add(ProgressBar::new(0));
		pb.set_style(
			ProgressStyle::default_spinner()
				.template(
					&format!("[{{elapsed_precise}}] [Thread {}] {{spinner}} {{msg}}", thread)
				)
				.expect("Failed to create template")
				.tick_chars("/|\\- "),
		);
		pb.set_message("Starting...");
		handles.insert(thread, pb);
	}

	repos.iter().par_bridge().for_each(|(full_name, stars, rust_percentage)| {
		let current_thread_id = rayon::current_thread_index().unwrap_or(0);
		let bar: &ProgressBar = handles.get(&current_thread_id).unwrap();
		bar.set_message(format!("Analyzing {} ({} stars, {:.2}% Rust)", full_name, stars, rust_percentage));
		run_analysis_for_repo(full_name, output_dir, bar);
		overall_bar.inc(1);
	});
	overall_bar.finish_with_message("Analysis completed.");
}

pub async fn run_collection_pipeline(
	personal_token: &str,
	max_stars: Option<u32>,
	output: &str,
) {
	
	let instance = octocrab::Octocrab::builder()
		.personal_token(personal_token)
		.build().unwrap();

	let mut stars = max_stars.unwrap_or(10_000_000);
	let mut page_num = 1u32;
	let mut i = 0;
	let mut previous_repositories: Vec<String> = Vec::new();
	let mut current_repositories: Vec<String> = Vec::new();

	loop {
		println!("Fetching repositories with up to {} stars...", stars);
		let page: octocrab::Page<octocrab::models::Repository> = instance
			.search()
			.repositories(&format!("stars:<={}", stars))
			.sort("stars")
			.order("desc")
			.per_page(100)
			.page(page_num)
			.send()
			.await.expect("Help");
		let mut changed = false;
		let old_stars = stars;

		println!("Processing {} repositories from page {}...", page.items.len(), page_num);

		let mut wtr = csv::WriterBuilder::new()
		.has_headers(false) // Avoid writing headers again
		.from_writer(std::fs::OpenOptions::new()
			.append(true)
			.create(true)
			.open(output)
			.expect("Failed to open CSV file"));

		for repo in page.items {
			let full_name = repo.full_name.unwrap();
			current_repositories.push(full_name.clone());

			if repo.stargazers_count.is_none() {
				println!("E({}-{}) {} -> no stars info.", i, 0, full_name);
				continue;
			}

			if previous_repositories.contains(&full_name) {
				println!(",({}-{}) {} -> already processed.", i, repo.stargazers_count.unwrap_or(0), full_name);
				tokio::time::sleep(std::time::Duration::from_millis(500)).await;
				continue;
			}

			stars = repo.stargazers_count.unwrap_or(0);
			changed |= stars < old_stars;

			let languages = instance
				.repos(&repo.owner.unwrap().login, &repo.name)
				.list_languages()
				.await.expect("Failed to fetch languages");

			let max_lang = languages.iter().max_by_key(|entry| entry.1).map(|(lang, _)| lang.clone()).unwrap_or("Unknown".to_string());
			i += 1;

			if !languages.contains_key("Rust") {
				println!(".({}-{}) {} -> no Rust (max: {}).", i, repo.stargazers_count.unwrap_or(0), full_name, max_lang);
				continue;
			}

			let code_sum = languages.values().sum::<i64>() as f64;
			let rust_percentage = (languages.get("Rust").unwrap_or(&0).clone() as f64 / code_sum) * 100.0;

			println!("+({}-{}) {} -> {:.2}% Rust (max: {})", i, repo.stargazers_count.unwrap_or(0), full_name, rust_percentage, max_lang);
			wtr.write_record(&[
				max_lang, 
				full_name, 
				repo.stargazers_count.unwrap_or(0).to_string(), 
				format!("{:.2}", rust_percentage),
				format!("{:?}", languages)
			])
				.expect("Failed to write record");

			wtr.flush().expect("Failed to flush CSV writer");
		}

		println!("Decrease stars threshold from {} to {} for next page (= {}).", old_stars, stars, old_stars - stars);

		if !changed {
			page_num += 1;
		} else {
			page_num = 1;
		}

		previous_repositories = current_repositories.clone();
		current_repositories.clear();
	}
}

#[derive(Debug)]
pub enum SymbolsError {
	ResultsDirError,
	CloneError,
	SymbolsGatherError,
	ResultsWriteError,
}

pub fn run_symbols_for_repo(
	repo: &Repo,
	output: &str,
	bar: &ProgressBar,
) -> Result<usize, SymbolsError> {
	let repo_name = repo.name.replace("/", "_");
	let temp_folder = format!("temp_symbols/{}", repo_name);
	let results_folder = format!("{}/{}", output, repo_name);
	let metadata_path = format!("{}/metadata.txt", results_folder);

	if Path::new(&metadata_path).exists() {
		return Ok(0);
	}

	if repo.size > 5_000_000 {
		eprintln!("[{}] Repository too large ({} KB), skipping.", repo_name, repo.size);
		return Err(SymbolsError::CloneError);
	}

	// if Path::new(&temp_folder).exists() {
	// 	fs::remove_dir_all(&temp_folder).map_err(|_| SymbolsError::ClearTempDirError)?;
	// }

	bar.set_message(format!("{}: Cloning", repo_name));
	let branch = &repo.main_branch;

	let git_url = format!("git@github.com:{}.git", repo.name);
	// let git_url = format!("https://github.com/{}.git", repo.name);

	let start_clone_time = std::time::Instant::now();
	let status = std::process::Command::new("git")
		.args(&["clone", &git_url, "--depth", "1", "--branch", branch, &temp_folder])
		.stdout(std::process::Stdio::null())
		.stderr(std::process::Stdio::null())
		.status()
		.map_err(|_| SymbolsError::CloneError)?;

	if !status.success() {
		eprintln!("[{}] Failed to clone repository with code {}", repo_name, status.code().unwrap_or(-1));
		return Err(SymbolsError::CloneError);
	}

	let commit_id = {
		let output = std::process::Command::new("git")
			.args(&["rev-parse", "HEAD"])
			.current_dir(&temp_folder)
			.output()
			.map_err(|_| SymbolsError::ResultsWriteError)?;

		if !output.status.success() {
			"unknown".to_string()
		} else {
			String::from_utf8_lossy(&output.stdout).trim().to_string()
		}
	};
	let clone_duration = start_clone_time.elapsed();

	let start_gather_time = std::time::Instant::now();
	let temp_folder_path = Path::new(&temp_folder);

	if !Path::new(&results_folder).exists() {
		fs::create_dir_all(&results_folder).map_err(|_| SymbolsError::ResultsDirError)?;
	}
	bar.set_message(format!("{}: Gathering", repo_name));

	let total_symbols = code::extensive_find_symbols(temp_folder_path, 1_000_000, &|index: usize, symbols_result: HashMap<SupportedLanguage, HashSet<Box<SymbolData>>>| -> Result<(), SymbolsError> {
		let total_symbols: usize = symbols_result.values().map(|symbols| symbols.len()).sum();
		if total_symbols == 0 {
			return Ok(());
		}
		let mut result_str = String::with_capacity(total_symbols * 100); // Estimate average symbol length + other fields
		result_str.push_str("Language,File,Start,Name,ParentKind,GrandparentKind,GreatGrandparentKind\n");

		for (lang, symbols) in &symbols_result {
			for symbol in symbols {
				result_str.push_str(&format!("{},{},{},{},{},{},{}\n", lang.to_string(), symbol.path, symbol.start, symbol.name, symbol.parent_kind.clone().unwrap_or("".to_string()), symbol.grandparent_kind.clone().unwrap_or("".to_string()), symbol.great_grandparent_kind.clone().unwrap_or("".to_string())));
			}
		}
		let symbols_file = format!("{}/symbols_{}.csv.gz", results_folder, index);
		let gz_file = fs::File::create(&symbols_file)
			.map_err(|_| SymbolsError::ResultsWriteError)?;
		let mut encoder = flate2::write::GzEncoder::new(gz_file, flate2::Compression::default());
		encoder
			.write_all(result_str.as_bytes())
			.map_err(|_| SymbolsError::ResultsWriteError)?;
		encoder.finish().map_err(|_| SymbolsError::ResultsWriteError)?;
		bar.inc(1);
		bar.set_message(format!("{}: Gathered symbols batch {} ({} symbols)", repo_name, index + 1, total_symbols));

		Ok(())
	}).map_err(|_| SymbolsError::SymbolsGatherError)?;

	let gather_duration = start_gather_time.elapsed();

	let metadata_file = fs::File::create(&metadata_path)
		.map_err(|_| SymbolsError::ResultsWriteError)?;
	let mut metadata_writer = std::io::BufWriter::new(metadata_file);
	metadata_writer
		.write_all(format!(
			"Name: {}\nStars: {}\nForks: {}\nMain Branch: {}\nCreated At: {}\nIs Fork: {}\nLicense: {}\n\nAnalyzed at: {}\nCommit id: {}\n\nCloned in: {:.2?}\nGathererd and wrote results in: {:.2?}\n\nTotal Symbols: {}\n",
			repo.name,
			repo.stars,
			repo.forks,
			repo.main_branch,
			repo.created_at,
			repo.is_fork,
			repo.license,
			chrono::Utc::now(),
			commit_id,
			clone_duration,
			gather_duration,
			total_symbols
		).as_bytes())
		.map_err(|_| SymbolsError::ResultsWriteError)?; 
	bar.set_message(format!("{}: Completed analysis ({} symbols)", repo_name, total_symbols));

	Ok(total_symbols)
}

pub struct Repo {
	pub name: String,
	pub stars: u32,
	pub forks: u32,
	pub main_branch: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	pub is_fork: bool,
	pub size: u32,
	pub language: String,
	pub license: String,
}

pub fn run_symbols_pipeline(
	input: &str,
	output: &str,
) {
	let repositories = csv::Reader::from_path(input)
		.expect("Failed to open repositories CSV file")
		.records()
		.enumerate()
		.map(|(i, result)| {
			let record = result.expect("Failed to read repository record");
			let name = record[0].to_string();
			let stars: u32 = record[1].parse().unwrap_or(0);
			let forks: u32 = record[2].parse().unwrap_or(0);
			let main_branch: String = record[3].to_string();
			let created_at: chrono::DateTime<chrono::Utc> = record[4].parse().unwrap_or(chrono::Utc::now());
			let updated_at: chrono::DateTime<chrono::Utc> = record[5].parse().unwrap_or(chrono::Utc::now());
			let is_fork: bool = record[6].parse().unwrap_or(false);
			let size: u32 = record[7].parse().unwrap_or(0);
			let language: String = record[8].to_string();
			let license: String = record[9].to_string();

			Repo { name, stars, forks, main_branch, created_at, updated_at, is_fork, size, language, license }
		})
		.collect::<Vec<Repo>>();

	let mut name_set = HashSet::new();
	let mut unique_repos = Vec::new();

	for repo in repositories {
		if !name_set.contains(&repo.name) {
			name_set.insert(repo.name.clone());
			unique_repos.push(repo);
		}
	}

	let mp = MultiProgress::new();
	let overall_bar = mp.add(ProgressBar::new(unique_repos.len() as u64));
    overall_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "[{elapsed_precise}] [{bar:100.cyan/blue}] {pos}/{len} ({eta})"
            )
            .expect("Failed to create template")
            .progress_chars("#>-"),
    );
	
	let mut handles: HashMap<usize, ProgressBar> = HashMap::new();
	for thread in 0..rayon::current_num_threads() {
		let pb = mp.add(ProgressBar::new(0));
		pb.set_style(
			ProgressStyle::default_spinner()
				.template(
					&format!("[{{elapsed_precise}}] [Thread {}] {{spinner}} {{msg}}", thread)
				)
				.expect("Failed to create template")
				.tick_chars("/|\\- "),
		);
		pb.set_message("Starting...");
		handles.insert(thread, pb);
	}

	unique_repos.sort_by(|a, b| b.stars.cmp(&a.stars));
	unique_repos.iter().par_bridge().for_each(|repo| {
		let current_thread_id = rayon::current_thread_index().unwrap_or(0);
		let bar: &ProgressBar = handles.get(&current_thread_id).unwrap();
		match run_symbols_for_repo(repo, output, bar) {
			Ok(n) => {
				overall_bar.inc(1);
			},
			Err(e) => {
				eprintln!("Error processing repository {}: {:?}", repo.name, e);
			}
		}
		fs::remove_dir_all(format!("temp_symbols/{}", repo.name.replace("/", "_"))).unwrap_or(());
	});

	overall_bar.finish_with_message("Collection completed.");
}

pub async fn run_symbols_collect_pipeline(
	min_stars: &u32,
) {
	let instance = octocrab::Octocrab::builder()
		.build().unwrap();

	let repositories_file = Path::new("results/all_repositories.csv");
	let mut repositories_writer = fs::OpenOptions::new()
		.append(true)
		.create(true)
		.open(&repositories_file)
		.expect("Failed to create result file");
	

	let mut stars = min_stars.clone();
	let mut page_num = 1u32;
	loop {
		let page: octocrab::Page<octocrab::models::Repository> = match instance
			.search()
			.repositories(&format!("stars:>={}", stars))
			.sort("stars")
			.order("asc")
			.per_page(100)
			.page(page_num)
			.send()
			.await 
		{
			Ok(p) => p,
			Err(e) => {
				eprintln!("Error fetching repositories: {}", e);
				println!("Sleeping for 1 minute before retrying...");
				tokio::time::sleep(std::time::Duration::from_secs(60)).await;
				continue;
			}
		};

		let highest_stars = page.items.iter()
			.filter_map(|repo| repo.stargazers_count)
			.max()
			.unwrap_or(0);

		println!("Processing {} repositories with {} stars from page {}...", page.items.len(), stars, page_num);

		if page.items.len() == 0 {
			println!("Done!");
			return;
		}

		for repo in &page.items {
			let full_name = match repo.full_name.as_ref() {
				Some(name) => name,
				None => {
					eprintln!("Repository without full name, skipping.");
					continue;
				}
			};
			let stars_count = repo.stargazers_count.unwrap_or(0);
			let forks_count = repo.forks_count.unwrap_or(0);
			let main_branch = if repo.default_branch.is_some() {
				repo.default_branch.as_ref().unwrap()
			} else {
				"main"
			};
			let is_fork = repo.fork.unwrap_or(false);
			let created_at = repo.created_at.unwrap_or(chrono::Utc::now());
			let updated_at = repo.updated_at.unwrap_or(chrono::Utc::now());
			let size = repo.size.unwrap_or(0);
			let language = repo.language.as_ref()
				.and_then(|value| value.as_str())
				.unwrap_or("Unknown");
			let license = match &repo.license {
				Some(lic) => lic.spdx_id.clone(),
				None => "None".to_string(),
			};
			repositories_writer
				.write_all(format!("{},{},{},{},{},{},{},{},{},{}\n", full_name, stars_count, forks_count, main_branch, created_at, updated_at, is_fork, size, language,license).as_bytes())
				.expect("Failed to write to repositories file");
		}

		if stars == highest_stars {
			page_num += 1;
		} else {
			page_num = 1;
		}
		stars = highest_stars;

		tokio::time::sleep(std::time::Duration::from_secs(6)).await;
	}
}

pub fn run_symbols_hash_pipeline(
	input: &str,
	output: &str,
) {
	if Path::new(output).exists() {
		fs::remove_file(output).expect("Failed to clear output file");
	}

	let input_path = Path::new(input);
	let folders: Vec<_> = fs::read_dir(input_path)
        .expect("Failed to read input directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();

	let total_size = folders.len();
	let mp = MultiProgress::new();
	let overall_bar = mp.add(ProgressBar::new(total_size as u64));
    overall_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "[{elapsed_precise}] [{bar:100.cyan/blue}] {pos}/{len} ({eta})"
            )
            .expect("Failed to create template")
            .progress_chars("#>-"),
    );

	let mut handles: HashMap<usize, ProgressBar> = HashMap::new();
	for thread in 0..rayon::current_num_threads() {
		let pb = mp.add(ProgressBar::new(0));
		pb.set_style(
			ProgressStyle::default_spinner()
				.template(
					&format!("[{{elapsed_precise}}] [Thread {}] {{spinner}} {{msg}}", thread)
				)
				.expect("Failed to create template")
				.tick_chars("/|\\- "),
		);
		pb.set_message("Starting...");
		handles.insert(thread, pb);
	}

	if Path::new(output).exists() {
		fs::remove_file(output).expect("Failed to clear output file");
	}
	
	let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(output)
        .expect("Failed to open output file");
    
    let writer = Arc::new(Mutex::new(BufWriter::new(file)));
	
	folders.par_iter().for_each(|dir| {
		let current_thread_id = rayon::current_thread_index().unwrap_or(0);
		let bar: &ProgressBar = handles.get(&current_thread_id).unwrap();
mp.suspend(|| {
		hash::hash_for_project(dir.to_path_buf(), &writer, bar);
		});
		overall_bar.inc(1);
	});

	writer.lock().unwrap().flush().unwrap();

	overall_bar.finish_with_message("Hashing completed.");
}