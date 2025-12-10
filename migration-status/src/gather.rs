use std::{fs, panic, thread};
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::{Arc, mpsc, Mutex};
use std::time::{Duration, Instant};

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
    func: &mut dyn FnMut(usize, &String) -> bool,
)-> Result<(), GatherError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| GatherError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(&bar, owner, repo, &temp_repo_dir)
            .map_err(|_| GatherError::RepositoryCloneError)?;

    bar.set_message(format!("{}_{}: Getting main branch", owner, repo));
    bar.inc(1);
    let main_branch = repo_info
        .get_main_branch()
        .ok_or(GatherError::MainBranchNotFound)?;

    let commits = repo_info
        .get_commits(&main_branch)
        .map_err(|_| GatherError::RevwalkError)?;

    let commit_count = commits.len();
    bar.set_message(format!("{}_{}: Branch {} has {} commits", owner, repo, main_branch, commit_count));

    if commit_count == 0 {
        return Ok(());
    }

    let indices = sample_indices(commit_count, num_commits).into_iter().rev().collect::<Vec<_>>();
    // let func_arc = Arc::clone(&func);

    for (i, commit_index) in indices.into_iter().enumerate() {
        let commit_id = commits[commit_index];

        let commit = match repo_info.checkout_commit(commit_id, &main_branch) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // bar.set_message(format!("{}: {}_{} - Checked out commit {} at {}", i, owner, repo, &commit.id().to_string()[..8], chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error")));
        bar.inc(1);

        func(i, &temp_repo_dir);
        // let (tx, rx) = mpsc::channel();
        // let func_clone = Arc::clone(&func_arc);
        // let dir_clone = temp_repo_dir.clone();

        // thread::spawn(move || {
        //     let result = panic::catch_unwind(AssertUnwindSafe(|| {
        //         func_clone(i, &dir_clone)
        //     }));
            
        //     let _ = tx.send(result);
        // });

        // let should_continue = match rx.recv_timeout(Duration::from_secs(15 * 60)) {
        //     // Case 1: Function completed successfully
        //     Ok(Ok(result)) => {
        //         bar.set_message(format!("{}: {}_{} - Finished gathering data", i, owner, repo));
        //         result
        //     },
            
        //     // Case 2: Function panicked (crashed)
        //     Ok(Err(_panic_payload)) => {
        //         bar.set_message(format!("{}: {}_{} - WORKER PANICKED (skipping)", i, owner, repo));
        //         // You could optionally log _panic_payload here if you want details
        //         true // Continue to next commit
        //     },

        //     // Case 3: Timeout reached
        //     Err(mpsc::RecvTimeoutError::Timeout) => {
        //         bar.set_message(format!("{}: {}_{} - TIMED OUT (skipping)", i, owner, repo));
        //         true // Continue to next commit
        //     },

        //     // Case 4: Channel disconnected without sending (rare, usually covered by panic catch above)
        //     Err(mpsc::RecvTimeoutError::Disconnected) => {
        //         bar.set_message(format!("{}: {}_{} - Thread disconnected unexpectedly", i, owner, repo));
        //         true
        //     }
        // };
        bar.set_message(format!("{}: {}_{} - Finished gathering data", i, owner, repo));
        bar.inc(1);

        // if !should_continue {
        //     break;
        // }
    }

    Ok(())
}

pub struct AnalyzeResult {
    pub lang_stats: HashMap<LanguageType, (f64, usize, usize, usize)>,
    pub symbols: HashSet<code::SupportedLanguage>,
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

    let language_map = code::find_symbols(&result_dir, index, path).unwrap_or_default();

    let mut languages = Languages::new();
    languages.get_statistics(&paths, &excluded, &config);
    let mut lang_stats = HashMap::new();
    let total_loc = languages.total().code as f64;
    for (lang, stats) in languages.iter() {
        let loc = stats.code;
        let loc_pct = loc as f64 / total_loc.max(1.0);
        lang_stats.insert(*lang, (loc_pct, loc, stats.blanks, stats.comments));
    }

    AnalyzeResult {
        lang_stats,
        symbols: language_map,
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

    // let symbols_arc: Arc<Mutex<Vec<HashSet<code::SupportedLanguage>>>> = Arc::new(Mutex::new(Vec::new()));
    // let lang_stats_arc: Arc<Mutex<Vec<HashMap<LanguageType, (f64, usize, usize, usize)>>>> = Arc::new(Mutex::new(Vec::new()));

    // let res_dir = result_dir.to_string();
    // let symbols_clone = Arc::clone(&symbols_arc);
    // let lang_stats_clone = Arc::clone(&lang_stats_arc);

    // let func: Arc<dyn Fn(usize, &String) -> bool + Send + Sync + 'static> = Arc::new(move |i: usize, dir: &String| {
    //     let analyze_result = analyze_commit(&res_dir, i, dir);
    //     symbols_clone.lock().unwrap().push(analyze_result.symbols);
    //     lang_stats_clone.lock().unwrap().push(analyze_result.lang_stats);
    //     true
    // });
    let mut symbols = Vec::new();
    let mut lang_stats = Vec::new();

    select_evenly_spread_commits_and_checkout_each(owner, repo, temp_dir, num_commits, bar, &mut |i: usize, dir: &String| {
        let analyze_result = analyze_commit(&result_dir, i, dir);
        symbols.push(analyze_result.symbols);
        lang_stats.push(analyze_result.lang_stats);
        true
    })?;

    // let symbols = symbols_arc.lock().unwrap().clone();
    // let lang_stats = lang_stats_arc.lock().unwrap().clone();

    Ok(RepositoryStats {
        length: lang_stats.len(),
        name: repo_name,
        lang_stats,
        symbols,
        results_folder: result_dir.to_string(),
    })
}