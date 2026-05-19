use std::{fs, panic, thread};
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::{Arc, mpsc, Mutex};
use std::time::{Duration, Instant};

use indicatif::ProgressBar;
use tokei::{Config, LanguageType, Languages};
use chrono;
use std::collections::{HashMap, HashSet};
use crate::code::{self, SupportedLanguage, SymbolData};

use crate::pipeline::SymbolsError;
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
    func: &mut dyn FnMut(usize, &String, &git2::Commit) -> bool,
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

        func(i, &temp_repo_dir, &commit);
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

fn clean_text(text: Option<String>) -> String {
    text.map(|s| s.replace('\n', " ").replace('\r', " ")).unwrap_or_default()
}

pub fn analyze_commit(
    result_dir: &str,
    index: usize,
    dir: &String,
) -> AnalyzeResult {
    let paths = [dir.as_str()];
    let excluded: [&str; 0] = [];
    let config = Config::default();

    code::extensive_find_symbols(Path::new(dir), 1_000_000_000, &|i: usize, symbols_result: HashMap<SupportedLanguage, HashSet<Box<SymbolData>>>| -> Result<(), SymbolsError> {
        let symbols_file = format!("{}/{}_symbols.csv.gz", result_dir, index);
        let gz_file = fs::File::create(&symbols_file)
			.map_err(|_| SymbolsError::ResultsWriteError)?;
        let mut encoder = flate2::write::GzEncoder::new(gz_file, flate2::Compression::default());

        writeln!(
            encoder,
            "Language,File,Start,Name,Field,ParentKind,ParentField,GrandparentKind,GrandparentField,GreatGrandparentKind,GreatGrandparentField"
        ).map_err(|_| SymbolsError::ResultsWriteError)?;

        for (lang, symbols) in symbols_result {
            for symbol in symbols {
                writeln!(
                    encoder,
                    "{},{},{},{},{},{},{},{},{},{},{},{}",
                    lang.to_string(),
                    symbol.path,
                    symbol.start,
                    clean_text(Some(symbol.name)),
                    clean_text(Some(symbol.kind)),
                    clean_text(symbol.field),
                    clean_text(symbol.parent_kind),
                    clean_text(symbol.parent_field),
                    clean_text(symbol.grandparent_kind),
                    clean_text(symbol.grandparent_field),
                    clean_text(symbol.great_grandparent_kind),
                    clean_text(symbol.great_grandparent_field),
                ).map_err(|_| SymbolsError::ResultsWriteError)?;
            }
        }
        encoder.finish().map_err(|_| SymbolsError::ResultsWriteError)?;
        Ok(())
    }).unwrap_or_default();

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
        lang_stats: HashMap::new(),
        symbols: HashSet::new(),
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

    let mut commits_writer = fs::File::create(format!("{}/metadata.csv", result_dir)).expect("Failed to create commits file");
    writeln!(commits_writer, "index,commit_oid,commit_time,commit_summary").expect("Failed to write to commits file");

    select_evenly_spread_commits_and_checkout_each(owner, repo, temp_dir, num_commits, bar, &mut |i: usize, dir: &String, commit: &git2::Commit| {
        writeln!(
            commits_writer,
            "{},{},{},{}",
            i,
            commit.id().to_string(),
            chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            commit.summary().unwrap_or("").replace('\n', " ").replace('\r', " ")
        ).expect("Failed to write to commits file");
        let analyze_result = analyze_commit(&result_dir, i, dir);
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

pub fn gather_two_commit_stats(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    commit_indices: (usize, usize),
) -> Result<(HashMap<SupportedLanguage, HashSet<String>>, HashMap<SupportedLanguage, HashSet<String>>), GatherError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| GatherError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let bar = ProgressBar::new(0);
    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(&bar, owner, repo, &temp_repo_dir)
            .map_err(|_| GatherError::RepositoryCloneError)?;

    let main_branch = repo_info
        .get_main_branch()
        .ok_or(GatherError::MainBranchNotFound)?;

    let commits = repo_info
        .get_commits(&main_branch)
        .map_err(|_| GatherError::RevwalkError)?;

    let commit_count = commits.len();

    if commit_count == 0 {
        return Err(GatherError::RevwalkError);
    }

    let indices = sample_indices(commit_count, 100).into_iter().rev().collect::<Vec<_>>();

    let index1 = indices[commit_indices.0];
    let commit1 = match repo_info.checkout_commit(commits[index1], &main_branch) {
        Ok(c) => c,
        Err(_) => return Err(GatherError::RevwalkError),
    };
    let symbols1 = code::find_symbols_local(Path::new(&temp_repo_dir)).map_err(|_| GatherError::RevwalkError)?;

    let index2 = indices[commit_indices.1];
    let commit2 = match repo_info.checkout_commit(commits[index2], &main_branch) {
        Ok(c) => c,
        Err(_) => return Err(GatherError::RevwalkError),
    };
    let symbols2 = code::find_symbols_local(Path::new(&temp_repo_dir)).map_err(|_| GatherError::RevwalkError)?;

    Ok((symbols1, symbols2))
}