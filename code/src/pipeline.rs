use std::{fs::{File, create_dir_all}, io::Write, path::Path};

use common::{input::InputData, repository::RepositoryInfo};
use indicatif::ProgressBar;

use crate::{collect::{collect_repository, save_components}, commits, szz, truck::{self, TruckStats}};

pub const MAX_SAMPLES: usize = 250;

#[derive(Debug)]
pub enum PipelineError {
	RepositoryError,
	FileError,
}

fn sample_indices(total: usize, max_samples: usize) -> Vec<usize> {
    if total == 0 || max_samples == 0 {
        return Vec::new();
    }

    if total <= max_samples {
        return (0..total).collect();
    }

    let last = total - 1;
    (0..max_samples)
        .map(|i| i * last / (max_samples - 1))
        .collect()
}

pub fn run_pipeline(data: &InputData, bar: &ProgressBar, overall_bar: &ProgressBar) -> Result<(), PipelineError> {
    let name = format!("{}/{}", data.author, data.name);
    let results_folder = format!("results/{}_{}", data.author, data.name);
    if !Path::new(&results_folder).exists() {
		create_dir_all(&results_folder).map_err(|_| PipelineError::FileError)?;
	} else {
		let mut skipped_file_wtr = File::create("results/skipped.txt").map_err(|_| PipelineError::FileError)?;

		skipped_file_wtr.write_all(format!("Skipped: {}/{} at {}\n", data.author, data.name, chrono::Utc::now()).as_bytes()).map_err(|_| PipelineError::FileError)?;
		return Ok(());
	}
    let commits_file = format!("{}/commits.csv", results_folder);
	let mut commits_file_wtr = File::create(&commits_file).map_err(|_| PipelineError::FileError)?;
	commits_file_wtr.write_all(b"index,commit_oid,commit_time,checkout_ms,collect_ms,save_ms,truck_ms,commit_summary,message\n").map_err(|_| PipelineError::FileError)?;
	let mut truck_wtr = File::create(format!("{}/truck_stats.csv", results_folder)).map_err(|_| PipelineError::FileError)?;
	truck_wtr.write_all(TruckStats::header().as_bytes()).map_err(|_| PipelineError::FileError)?;

	let temp_dir = format!("temp/{}_{}", data.author, data.name);

	let repo = RepositoryInfo::clone_or_open(&data.author, &data.name, &temp_dir).map_err(|_| PipelineError::RepositoryError)?;

	szz::run(&bar, &name, &temp_dir, &results_folder);

	let main_branch = repo.get_main_branch().ok_or_else(|| PipelineError::RepositoryError)?;
	let mut commits = repo.get_commits(&main_branch).map_err(|_| PipelineError::RepositoryError)?;
	commits.reverse(); // Reverse to get oldest to newest

	commits::run(&commits, &repo.repository, &results_folder);

	bar.set_message(format!("Found {} commits in main branch", commits.len()));
	let indices = sample_indices(commits.len(), MAX_SAMPLES);
	let total_samples = indices.len();

	for (i, index) in indices.iter().enumerate() {
		let start_time = std::time::Instant::now();
		let commit_oid = commits[*index];
		let commit = match repo.checkout_commit(commit_oid) {
            Ok(c) => c,
            Err(err) => {
				bar.set_message(format!("{} [{}/{}] Failed to checkout commit {}: {}, skipping...", name, i + 1, total_samples, &commit_oid.to_string()[..8], err));
				commits_file_wtr.write_all(format!(
					"{},{},{},{},{},{},{},{},{},Checkout failed: {}\n", 
					i,
					commit_oid, 
					"",
					"",
					0,
					0,
					0,
					0,
					0,
					err
				).as_bytes()).map_err(|_| PipelineError::FileError)?;
				continue;
			}
        };
		let checkout_ms = start_time.elapsed().as_millis();
		bar.set_message(format!("{} [{}/{}] Checked out commit {} in {} ms, running analysis...", name, i + 1, total_samples, &commit.id().to_string()[..8], checkout_ms));

		let start_time = std::time::Instant::now();
		let result = collect_repository(&temp_dir);

		let analysis_ms = start_time.elapsed().as_millis();

		if let Err(err) = result {
			bar.set_message(format!("{} [{}/{}] Analysis failed after {} ms, continuing...", name, i + 1, total_samples, analysis_ms));
			
			commits_file_wtr.write_all(format!(
				"{},{},{},{},{},{},{},{},{},,Analysis failed because {:?}\n", 
				i,
				commit_oid, 
				chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
				checkout_ms,
				analysis_ms,
				0,
				0,
				0,
				commit.summary().unwrap_or(""),
				err
			).as_bytes()).map_err(|_| PipelineError::FileError)?;
			continue;
		} else {
			bar.set_message(format!("{} [{}/{}] Analysis completed after {} ms, saving...", name, i + 1, total_samples, analysis_ms));
		}

		let start_time = std::time::Instant::now();
		let results_file = format!("{}/{}.json.zip", results_folder, i);
		save_components(&Path::new(&results_file), &result.unwrap()).map_err(|_| PipelineError::FileError)?;
		let save_ms = start_time.elapsed().as_millis();

		let start_time = std::time::Instant::now();
		let stats = truck::get_truck_statistics(&temp_dir);
		let truck_ms = start_time.elapsed().as_millis();

		truck_wtr.write_all(stats.to_csv(i, &commit_oid.to_string()).as_bytes()).map_err(|_| PipelineError::FileError)?;

		commits_file_wtr.write_all(format!(
			"{},{},{},{},{},{},{},{},\n", 
			i,
			commit_oid, 
			chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
			checkout_ms,
			analysis_ms,
			save_ms,
			truck_ms,
			commit.summary().unwrap_or(""),
		).as_bytes()).map_err(|_| PipelineError::FileError)?;
		overall_bar.inc(1);
	}

	Ok(())
}