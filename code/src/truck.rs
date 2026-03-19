use std::path::Path;

use truck_facto_rs::{git, file, doa, tf, gini};

pub struct TruckStats {
    num_commits: usize,
    num_files: usize,
    num_vendored_files: usize,
    num_authors_merged: usize,
    num_authors: usize,
    num_doa_authors: usize,
    num_doa_files: usize,
    truck_factor: u64,
    gini_coefficient: f64,
}

impl TruckStats {
    pub fn to_csv(&self, index: usize, commit: &str) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            index,
            commit,
            self.num_commits,
            self.num_files,
            self.num_vendored_files,
            self.num_authors_merged,
            self.num_authors,
            self.num_doa_authors,
            self.num_doa_files,
            self.truck_factor,
            self.gini_coefficient,
        )
    }

    pub fn header() -> String {
        "index,commit,num_commits,num_files,num_vendored_files,num_authors_merged,num_authors,num_doa_authors,num_doa_files,truck_factor,gini_coefficient\n".to_string()
    }
}

pub fn get_truck_statistics(repo_path_str: &str) -> TruckStats {
    let repo_path = Path::new(repo_path_str);
    let repo = git::Repo { path: repo_path };
    
    let mut commits = git::get_commit_info(&repo);
    git::populate_files_for_commits(&repo, &mut commits);
    let (num_authors_merged, num_authors) = git::merge_alias_authors(&mut commits);

    let mut files = file::get_files_in_repo(&repo);
    file::mark_vendored_files(&mut files, &repo);

    let file_names: Vec<String> = files.iter().filter(|f| !f.filtered).map(|f| f.name.clone()).collect();
    git::assign_recent_names(&file_names, &mut commits);

    let doa_files = doa::prepare_for_doa(&file_names, &commits);

    let tf = tf::calculate_truck_factor(&doa_files);
    let gini = gini::calculate_gini(&doa_files);

    TruckStats {
        num_commits: commits.len(),
        num_files: files.len(),
        num_vendored_files: files.iter().filter(|f| f.filtered).count(),
        num_authors_merged,
        num_authors,
        num_doa_authors: tf.1.len(),
        num_doa_files: doa_files.len(),
        truck_factor: tf.0,
        gini_coefficient: gini,
    }
}