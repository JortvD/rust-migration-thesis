use crate::input::InputData;
use crate::project::Project;
use crate::repository::BareRepositoryInfo;
use std::io::Write;
use std::fs::{File, create_dir_all};
use std::path::Path;
use indicatif::ProgressBar;

const MAX_SAMPLES: usize = 10; // 250

#[derive(Debug)]
pub enum PipelineError {
	RepositoryError(git2::Error),
	FileError(std::io::Error),
	ProjectError,
	CommandError(String),
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
		create_dir_all(&results_folder).map_err(PipelineError::FileError)?;
	}
	let commits_file = format!("{}/commits.csv", results_folder);
	let mut commits_file_wtr = File::create(&commits_file).map_err(PipelineError::FileError)?;

	let project = Project::create(&data.name, &results_folder).map_err(|_| PipelineError::ProjectError)?;

	let temp_dir = format!("temp/{}_{}", data.author, data.name);

	let repo = BareRepositoryInfo::clone_or_open(&bar, &data.author, &data.name, &temp_dir).map_err(PipelineError::RepositoryError)?;
	let main_branch = repo.get_main_branch().ok_or_else(|| PipelineError::RepositoryError(git2::Error::from_str("No main branch found")))?;
	let commits = repo.get_commits(&main_branch).map_err(PipelineError::RepositoryError)?;
	bar.set_message(format!("Found {} commits in main branch", commits.len()));
	let indices = sample_indices(commits.len(), MAX_SAMPLES);

	for (i, index) in indices.iter().enumerate() {
		let commit_oid = commits[*index];
		let commit = match repo.checkout_commit(commit_oid, &main_branch) {
            Ok(c) => c,
            Err(_) => continue,
        };

		bar.set_message(format!("{} [{}/{}] Checked out commit {}", name, i + 1, indices.len(), &commit.id().to_string()[..8]));

		commits_file_wtr.write_all(format!(
			"{},{},{}\n", 
			commit_oid, 
			chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
			commit.summary().unwrap_or("")
		).as_bytes()).map_err(PipelineError::FileError)?;

		let start_time = std::time::Instant::now();

		let result = project.run_analysis(&repo.dir, i).map_err(|_| PipelineError::ProjectError)?;
		let analysis_secs = start_time.elapsed().as_secs();
		bar.set_message(format!("{} [{}/{}] Analysis completed with exit code {} in {} seconds", name, i + 1, indices.len(), result, analysis_secs));

		std::thread::sleep(std::time::Duration::from_secs(analysis_secs / 10));

		let result = project.get_results(i).map_err(|_| PipelineError::ProjectError)?;
		bar.set_message(format!("{} [{}/{}] Results retrieved: {} items", name, i + 1, indices.len(), result));
	}

	project.delete().map_err(|_| PipelineError::ProjectError)?;

	Ok(())
}