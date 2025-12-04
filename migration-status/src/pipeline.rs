use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic;
use std::sync::atomic::AtomicUsize;

use rayon::prelude::*;

use crate::gather;
use crate::analyze;

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

const NUM_COMMITS: usize = 100;

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
	let i = Arc::new(AtomicUsize::new(0));

	repos.iter().par_bridge().for_each(|(full_name, stars, rust_percentage)| {
		let current_index = i.fetch_add(1, atomic::Ordering::SeqCst);
		println!(
			"[{}/{}] Analyzing repository: {} ({} stars, {:.2}% Rust)",
			current_index + 1,
			total_repos,
			full_name,
			stars,
			rust_percentage
		);

		let status = std::process::Command::new("./target/release/migration-status")
			.args(&["single", full_name, &output_dir])
			.status()
			.expect("Failed to execute cargo command");

		if !status.success() {
			eprintln!("Failed to analyze repository: {}", full_name);
		}
	});

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
