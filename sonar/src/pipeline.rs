use crate::input::InputData;
use crate::project::Project;
use crate::repository::BareRepositoryInfo;
use std::io::Write;
use std::fs::{File, create_dir_all};
use std::path::Path;
use indicatif::ProgressBar;

const MAX_SAMPLES: usize = 250; // 250

#[derive(Debug)]
pub enum PipelineError {
	RepositoryError,
	FileError,
	ProjectError,
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

pub fn run_pipeline(data: &InputData, bar: &ProgressBar) -> Result<(), PipelineError> {
	let name = format!("{}/{}", data.author, data.name);
	bar.set_message(format!("Cloning {}/{}", data.author, data.name));
	let results_folder = format!("results/{}_{}", data.author, data.name);
	if !Path::new(&results_folder).exists() {
		create_dir_all(&results_folder).map_err(|_| PipelineError::FileError)?;
	} else {
		bar.set_message(format!("Results folder already exists for {}/{}", data.author, data.name));
		let mut skipped_file_wtr = File::create("results/skipped.txt").map_err(|_| PipelineError::FileError)?;

		skipped_file_wtr.write_all(format!("Skipped: {}/{} at {}\n", data.author, data.name, chrono::Utc::now()).as_bytes()).map_err(|_| PipelineError::FileError)?;
		return Ok(());
	}
	let commits_file = format!("{}/commits.csv", results_folder);
	let mut commits_file_wtr = File::create(&commits_file).map_err(|_| PipelineError::FileError)?;
	commits_file_wtr.write_all(b"commit_oid,commit_time,commit_summary,checkout_ms,analysis_ms,upload_ms,retrieval_ms\n").map_err(|_| PipelineError::FileError)?;

	let project = Project::create(&data.name, &results_folder).map_err(|_| PipelineError::ProjectError)?;

	let temp_dir = format!("temp/{}_{}", data.author, data.name);

	let repo = BareRepositoryInfo::clone_or_open(&bar, &data.author, &data.name, &temp_dir).map_err(|_| PipelineError::RepositoryError)?;
	let main_branch = repo.get_main_branch().ok_or_else(|| PipelineError::RepositoryError)?;
	let mut commits = repo.get_commits(&main_branch).map_err(|_| PipelineError::RepositoryError)?;
	commits.reverse(); // Reverse to get oldest to newest
	bar.set_message(format!("Found {} commits in main branch", commits.len()));
	let indices = sample_indices(commits.len(), MAX_SAMPLES);

	for (i, index) in indices.iter().enumerate() {

		let start_time = std::time::Instant::now();
		let commit_oid = commits[*index];
		let commit = match repo.checkout_commit(commit_oid, &main_branch) {
            Ok(c) => c,
            Err(_) => continue,
        };
		let checkout_ms = start_time.elapsed().as_millis();
		bar.set_message(format!("{} [{}/{}] Checked out commit {} in {} ms, running analysis...", name, i + 1, indices.len(), &commit.id().to_string()[..8], checkout_ms));

		let start_time = std::time::Instant::now();
		let result = project.run_analysis(&repo.dir, i).map_err(|_| PipelineError::ProjectError)?;
		let analysis_ms = start_time.elapsed().as_millis();

		if result != 0 {
			bar.set_message(format!("{} [{}/{}] Analysis failed with exit code {} in {} ms, skipping upload and retrieval...", name, i + 1, indices.len(), result, analysis_ms));
			continue;
		} else {
			bar.set_message(format!("{} [{}/{}] Analysis completed with exit code {} in {} ms, waiting for upload...", name, i + 1, indices.len(), result, analysis_ms));
		}
		
		let start_time = std::time::Instant::now();
		std::thread::sleep(std::time::Duration::from_secs(3));
		for _ in 0..1000 {
			if let Ok(num_completed) = project.get_activity_count() {
				bar.set_message(format!("{} [{}/{}] Waiting {} secs for upload... {}/{} activities completed", name, i + 1, indices.len(), start_time.elapsed().as_secs(), num_completed, indices.len()));
				if num_completed == i as u64 + 1 {
					break;
				}
			} else {
				bar.set_message(format!("{} [{}/{}] Waiting {} secs for upload... failed to get activity count", name, i + 1, indices.len(), start_time.elapsed().as_secs()));
			}
			std::thread::sleep(std::time::Duration::from_secs(1));
		}
		let upload_ms = start_time.elapsed().as_millis();

		let start_time = std::time::Instant::now();
		let result = project.get_results(i, bar).map_err(|_| PipelineError::ProjectError)?;
		bar.set_message(format!("{} [{}/{}] Retrieved {} items, checking out next...", name, i + 1, indices.len(), result));
		let retrieval_ms = start_time.elapsed().as_millis();

		commits_file_wtr.write_all(format!(
			"{},{},{},{},{},{},{}\n", 
			commit_oid, 
			chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
			commit.summary().unwrap_or(""),
			checkout_ms,
			analysis_ms,
			upload_ms,
			retrieval_ms
		).as_bytes()).map_err(|_| PipelineError::FileError)?;
	}

	project.delete().map_err(|_| PipelineError::ProjectError)?;

	Ok(())
}