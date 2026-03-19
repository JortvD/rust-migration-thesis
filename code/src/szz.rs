use std::{fs, path::Path, time::Instant};

use indicatif::ProgressBar;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct BugInducingCommit {
    hash: String,
    subject: String,
    date:u64,
    author_name: String,
    author_email: String,
    committer_name: String,
    committer_email: String,
    trivial: bool,
    fix: bool,
}

#[derive(Debug, Clone, Serialize)]
struct BugFixCommit {
    hash: String,
    subject: String,
    date: u64,
    author_name: String,
    author_email: String,
    committer_name: String,
    committer_email: String,
    inducing_commits: Vec<BugInducingCommit>,
    modified_files: Vec<szz_rs::szz::ModifiedFile>,
    skipped: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SzzResults {
    num_commits: usize,
    num_fix_related: usize,
    num_non_trivial: usize,
    num_non_merge: usize,
    results: Vec<BugFixCommit>,
}

pub fn run(bar: &ProgressBar, repo_name: &str, repo_path: &str, result_folder: &str) {
    let repo = szz_rs::git::Repo { path: Path::new(&repo_path) };
    let mut results = SzzResults {
        num_commits: 0,
        num_fix_related: 0,
        num_non_trivial: 0,
        num_non_merge: 0,
        results: Vec::new(),
    };

    let t = Instant::now();
    let commits = szz_rs::git::get_commits(&repo);
    bar.set_message(format!("[{}] Found {} commits in {:?}", repo_name, commits.len(), t.elapsed()));
    results.num_commits = commits.len();

    let t = Instant::now();
    let filtered = szz_rs::filter::filter_fix_related_word(commits.clone());
    bar.set_message(format!("[{}] Found {} fix-related commits in {:?}", repo_name, filtered.len(), t.elapsed()));
    results.num_fix_related = filtered.len();

    let t = Instant::now();
    let filtered = szz_rs::filter::filter_skip_trivial_fixes(filtered);
    bar.set_message(format!("[{}] Found {} non-trivial fix/bug-related commits in {:?}", repo_name, filtered.len(), t.elapsed()));
    results.num_non_trivial = filtered.len();

    let t = Instant::now();
    let filtered = szz_rs::filter::filter_skip_merge_commits(&repo, filtered);
    bar.set_message(format!("[{}] Found {} non-merge/non-trivial fix/bug-related commits in {:?}", repo_name, filtered.len(), t.elapsed()));
    results.num_non_merge = filtered.len();

    let mut fix_commits = Vec::new();

    for (i, commit) in filtered.iter().enumerate() {
        let t = Instant::now();
        let result = szz_rs::szz::szz_algorithm(&repo, &commit, &szz_rs::szz::SzzConfig::default());
        let (inducing_commits, modified_files) = result;
        let skipped = inducing_commits.is_none();
        let inducing_commits = inducing_commits.unwrap_or_default();

        let date = chrono::DateTime::from_timestamp(commit.author_date as i64, 0)
            .unwrap_or_else(|| panic!("Invalid timestamp: {}", commit.author_date));
        bar.set_message(format!("[{}/{}] found {} in {:?} for {} on {:?} by {}: {}", i + 1, filtered.len(), inducing_commits.len(), t.elapsed(), &commit.hash[..8], date, commit.author.name, commit.subject));

        fix_commits.push(BugFixCommit {
            hash: commit.hash.clone(),
            subject: commit.subject.clone(),
            date: commit.author_date,
            author_name: commit.author.name.clone(),
            author_email: commit.author.email.clone(),
            committer_name: commit.committer.name.clone(),
            committer_email: commit.committer.email.clone(),
            skipped,
            modified_files,
            inducing_commits: inducing_commits.into_iter().filter_map(|hash| {
                if let Some(inducing_commit) = commits.iter().find(|p| p.hash == hash) {
                    Some(BugInducingCommit {
                        hash: inducing_commit.hash.clone(),
                        subject: inducing_commit.subject.clone(),
                        date: inducing_commit.author_date,
                        author_name: inducing_commit.author.name.clone(),
                        author_email: inducing_commit.author.email.clone(),
                        committer_name: inducing_commit.committer.name.clone(),
                        committer_email: inducing_commit.committer.email.clone(),
                        trivial: szz_rs::filter::is_trivial_subject(&inducing_commit.subject),
                        fix: szz_rs::filter::is_fix_related_subject(&inducing_commit.subject),
                    })
                } else {
                    None
                }
            }).collect(),
        });
    }
    results.results = fix_commits;

    fs::write(format!("{}/szz.json", result_folder), serde_json::to_string_pretty(&results).unwrap()).expect("Failed to write output");
}