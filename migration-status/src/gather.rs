use std::fs;
use std::path::Path;
use std::time::Instant;

use indicatif::ProgressBar;
use tokei::{Config, LanguageType, Languages};
use chrono;
use std::collections::{HashMap, HashSet};
use crate::code::{self, SupportedLanguage};

use crate::repository;

#[derive(Debug)]
pub enum GatherError {
    TempDirCreationError,
    RepositoryCloneError,
    MainBranchNotFound,
    RevwalkError,
}

pub fn get_repo_symbols(name: &str) -> Result<HashMap<SupportedLanguage, HashSet<String>>, ()> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() != 2 {
        //println!("[{}] Skipping invalid repository name", name);
        return Err(());
    }
    let owner = parts[0];
    let repo = parts[1];

    let temp = format!("temp/{}_{}", owner, repo);

    if !Path::new(&temp).exists() {
        fs::create_dir_all(&temp).map_err(|_| ())?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp, owner, repo);
    let writer = &mut std::io::stdout();

    let bar = ProgressBar::new(0);
    let repo_info = repository::BareRepositoryInfo::clone_or_open(&bar, owner, repo, &temp_repo_dir).map_err(|_| ())?;
    let main_branch = repo_info.get_main_branch().ok_or(())?;
    let commit = repo_info.get_latest_commit(&main_branch).map_err(|_| ())?;
    let commit = match repo_info.checkout_commit(commit, &main_branch) {
        Ok(c) => c,
        Err(_) => return Err(()),
    };
    let path = Path::new(&temp_repo_dir);
    // let (symbols, file_count) = code::find_symbols(path).unwrap_or_default();

    Ok(HashMap::new()) //symbols
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
    bar: &ProgressBar,
    func: &mut dyn FnMut(&git2::Commit, usize, &String) -> bool,
)-> Result<(), GatherError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| GatherError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    bar.set_message(format!("{}: Cloning repository", repo));
    bar.inc(1);
    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(&bar, owner, repo, &temp_repo_dir)
            .map_err(|_| GatherError::RepositoryCloneError)?;

    bar.set_message(format!("{}: Getting main branch", repo));
    bar.inc(1);
    let main_branch = repo_info
        .get_main_branch()
        .ok_or(GatherError::MainBranchNotFound)?;

    let start_time = Instant::now();

    let commits = repo_info
        .get_commits(&main_branch)
        .map_err(|_| GatherError::RevwalkError)?;

    let commit_count = commits.len();
    bar.set_message(format!("{}: Branch {} has {} commits", repo, main_branch, commit_count));

    if commit_count == 0 {
        return Ok(());
    }

    let indices = sample_indices(commit_count, num_commits).into_iter().rev().collect::<Vec<_>>();

    for (i, commit_index) in indices.into_iter().enumerate() {
        let commit_id = commits[commit_index];
        let start_time = Instant::now();

        let commit = match repo_info.checkout_commit(commit_id, &main_branch) {
            Ok(c) => c,
            Err(_) => continue,
        };

        bar.set_message(format!("{}: {}_{} - Checked out commit {} at {}", i, repo, owner, &commit.id().to_string()[..8], chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error")));
        bar.inc(1);

        let result = func(&commit, i, &temp_repo_dir);

        bar.set_message(format!("{}: {}_{} - Finished gathering data from commit {}", i, repo, owner, &commit.id().to_string()[..8]));
        bar.inc(1);

        if !result {
            break;
        }
    }

    Ok(())
}

pub struct AnalyzeResult {
    pub lang_stats: HashMap<LanguageType, (f64, usize, usize, usize)>,
    pub symbols: HashSet<code::SupportedLanguage>,
    pub can_continue: bool,
}

pub fn analyze_commit(
    result_dir: &str,
    index: usize,
    dir: &String,
) -> AnalyzeResult {
    let path = Path::new(dir);
    let paths = [dir.as_str()];
    let excluded: [&str; 0] = [];
    let config = Config::default();

    let start_time = Instant::now();
    let (language_map, symbol_count, file_count) = code::find_symbols(&result_dir, index, path).unwrap_or_default();
    let symbol_measure_duration = start_time.elapsed().as_millis();

    let start_time = Instant::now();
    let mut languages = Languages::new();
    languages.get_statistics(&paths, &excluded, &config);
    let mut lang_stats = HashMap::new();
    let total_loc = languages.total().code as f64;
    for (lang, stats) in languages.iter() {
        let loc = stats.code;
        let loc_pct = loc as f64 / total_loc.max(1.0);
        lang_stats.insert(*lang, (loc_pct, loc, stats.blanks, stats.comments));
    }
    let lang_measure_duration = start_time.elapsed().as_millis();

    AnalyzeResult {
        lang_stats,
        symbols: language_map,
        can_continue: true,
    }
}


pub struct RepositoryStats {
    pub length: usize,
    pub name: String,
    pub lang_stats: Vec<HashMap<LanguageType, (f64, usize, usize, usize)>>,
    pub symbols: Vec<HashSet<code::SupportedLanguage>>,
    pub results_folder: String,
}

pub fn gather_repository_statistics(
    owner: &str,
    repo: &str,
    result_dir: &str,
    temp_dir: &str,
    num_commits: usize,
    bar: &ProgressBar,
) -> Result<RepositoryStats, GatherError> {
    let repo_name = format!("{}/{}", owner, repo);
    let mut symbols = Vec::new();
    let mut lang_stats = Vec::new();

    select_evenly_spread_commits_and_checkout_each(owner, repo, temp_dir, num_commits, bar, &mut |commit, i, dir| {
        let analyze_result = analyze_commit(&result_dir, i, &dir);
        symbols.push(analyze_result.symbols);
        lang_stats.push(analyze_result.lang_stats);
        
        true
    })?;

    Ok(RepositoryStats {
        length: lang_stats.len(),
        name: repo_name,
        lang_stats,
        symbols,
        results_folder: result_dir.to_string(),
    })
}