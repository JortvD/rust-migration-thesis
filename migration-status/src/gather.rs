use std::fs;
use std::path::Path;
use std::time::Instant;

use tokei::{Config, LanguageType, Languages};
use chrono;
use std::collections::HashMap;
use crate::code::{self, Symbol};

use crate::consts::DEBUG;
use crate::repository;

#[derive(Debug)]
pub enum GatherError {
    TempDirCreationError,
    RepositoryCloneError,
    MainBranchNotFound,
    RevwalkError,
    CommitLookupError,
    CheckoutError,
}

fn sample_indices(total: usize, max_samples: usize) -> Vec<usize> {
    if total == 0 || max_samples == 0 {
        return Vec::new();
    }

    if total <= max_samples {
        return (0..total).collect();
    }

    // Integer spacing from 0 to total - 1, inclusive.
    let last = total - 1;
    (0..max_samples)
        .map(|i| i * last / (max_samples - 1))
        .collect()
}

fn select_evenly_spread_commits_and_checkout_each(
    owner: &str, 
    repo: &str, 
    temp_dir: &str, 
    num_commits: usize, 
    func: &mut dyn FnMut(&git2::Commit, usize, &String)
) -> Result<(), GatherError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| GatherError::TempDirCreationError)?;
    }

    let repo_name = format!("{}/{}", owner, repo);
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(owner, repo, &temp_repo_dir)
            .map_err(|_| GatherError::RepositoryCloneError)?;

    let main_branch = repo_info
        .get_main_branch()
        .ok_or(GatherError::MainBranchNotFound)?;

    if DEBUG { println!("[{repo_name}] Analyzing branch: {main_branch}"); }
    let start_time = Instant::now();

    let commits = repo_info
        .get_commits(&main_branch)
        .map_err(|_| GatherError::RevwalkError)?;

    let commit_count = commits.len();
    if DEBUG { println!(
        "[{repo_name}] Commits in branch {main_branch}: {} (in {} ms)",
        commit_count,
        start_time.elapsed().as_millis()
    ); }

    if commit_count == 0 {
        return Ok(());
    }

    let indices = sample_indices(commit_count, num_commits).into_iter().rev().collect::<Vec<_>>();

    for (i, commit_index) in indices.into_iter().enumerate() {
        let commit_id = commits[commit_index];

        let commit = match repo_info.checkout_commit(commit_id, &main_branch) {
            Ok(c) => c,
            Err(_) => continue,
        };

        func(&commit, i, &temp_repo_dir);
    }

    Ok(())
}

pub struct AnalyzeResult {
    pub lang_stats: HashMap<LanguageType, (f64, usize)>,
    pub symbols: HashMap<code::SupportedLanguage, Vec<Symbol>>,
}

pub fn analyze_commit(
    repo: &String,
    commit: &git2::Commit,
    index: usize,
    dir: &String,
) -> AnalyzeResult {
    let start_time = Instant::now();
    let path = Path::new(dir);
    let paths = [dir.as_str()];
    let excluded: [&str; 0] = [];
    let config = Config::default();

    let symbols = code::find_symbols(path).unwrap_or_default();

    let mut languages = Languages::new();
    languages.get_statistics(&paths, &excluded, &config);
    let mut lang_stats = HashMap::new();
    let total_loc = languages.total().code as f64;
    for (lang, stats) in languages.iter() {
        let loc = stats.code;
        let loc_pct = loc as f64 / total_loc.max(1.0);
        lang_stats.insert(*lang, (loc_pct, loc));
    }

    if DEBUG { println!(
        "[{}][{}] Commit {} at {} analyzed in {} ms",
        repo,
        index + 1,
        &commit.id().to_string()[..8],
        chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
        start_time.elapsed().as_millis()
    ); }

    AnalyzeResult {
        lang_stats,
        symbols,
    }
}


pub struct RepositoryStats {
    pub length: usize,
    pub name: String,
    pub lang_stats: Vec<HashMap<LanguageType, (f64, usize)>>,
    pub symbols: Vec<HashMap<code::SupportedLanguage, Vec<Symbol>>>,
}

pub fn gather_repository_statistics(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    num_commits: usize,
) -> Result<RepositoryStats, GatherError> {
    let repo_name = format!("{}/{}", owner, repo);
    let mut symbols = Vec::new();
    let mut lang_stats = Vec::new();

    select_evenly_spread_commits_and_checkout_each(owner, repo, temp_dir, num_commits, &mut |commit, i, dir| {
        let analyze_result = analyze_commit(&repo_name, commit, i, &dir);
        symbols.push(analyze_result.symbols);
        lang_stats.push(analyze_result.lang_stats);
    })?;

    Ok(RepositoryStats {
        length: lang_stats.len(),
        name: repo_name,
        lang_stats,
        symbols,
    })
}