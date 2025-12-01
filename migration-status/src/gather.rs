use std::fs;
use std::path::Path;
use std::time::Instant;

use tokei::{Config, LanguageType, Languages};
use chrono;
use walkdir::WalkDir;
use std::collections::HashSet;
use crate::code::{self, Symbol};

use crate::repository;

#[derive(Debug)]
pub enum AnalyzeError {
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

fn analyze_commits(owner: &str, repo: &str, temp_dir: &str, num_commits: usize, func: &mut dyn FnMut(&git2::Commit, usize) ) -> Result<(), AnalyzeError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| AnalyzeError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(owner, repo, &temp_repo_dir)
        // repository::RepositoryInfo::clone_or_open(owner, repo, &temp_repo_dir)
            .map_err(|_| AnalyzeError::RepositoryCloneError)?;

    let main_branch = repo_info
        .get_main_branch()
        .ok_or(AnalyzeError::MainBranchNotFound)?;

    println!("Analyzing branch: {main_branch}");
    let start_time = Instant::now();

    let commits = repo_info
        .get_commits(&main_branch)
        .map_err(|_| AnalyzeError::RevwalkError)?;

    let commit_count = commits.len();
    println!(
        "Commits in branch {main_branch}: {} (in {} ms)",
        commit_count,
        start_time.elapsed().as_millis()
    );

    if commit_count == 0 {
        return Ok(());
    }

    let indices = sample_indices(commit_count, num_commits).into_iter().rev().collect::<Vec<_>>();
    
    let start_time = Instant::now();

    for (i, commit_index) in indices.into_iter().enumerate() {
        let commit_id = commits[commit_index];
        let start_time = Instant::now();

        let commit = match repo_info.checkout_commit(commit_id, &main_branch) {
            Ok(c) => c,
            Err(_) => continue,
        };

        func(&commit, i);

        // println!(
        //     "[{}/{}] Commit {} at {} in {} ms",
        //     commit_count - commit_index,
        //     commit_count,
        //     &commit.id().to_string()[..8],
        //     chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
        //     start_time.elapsed().as_millis()
        // );
    }

    println!(
        "Analyzed {} commits in {} ms",
        num_commits,
        start_time.elapsed().as_millis()
    );

    Ok(())
}

pub struct TokeiStepStatistics {
    pub commit_date: chrono::DateTime<chrono::Utc>,
    pub languages: Languages,
}

pub struct TokeiStatistics {
    pub steps: Vec<TokeiStepStatistics>,
}

pub fn tokei(owner: &str, repo: &str, temp_dir: &str, num_commits: usize) -> Result<TokeiStatistics, AnalyzeError> {
    let mut steps = Vec::new();
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);
    let paths = [temp_repo_dir.as_str()];
    let excluded: [&str; 0] = [];
    let config = Config::default();
    let mut found_rust = false;

    analyze_commits(owner, repo, temp_dir, num_commits, &mut |commit, _i| {
        let start_time = Instant::now();
        let mut languages = Languages::new();
        languages.get_statistics(&paths, &excluded, &config);
        println!("Found {} LOC in {} ms", languages.total().code, start_time.elapsed().as_millis());
        if !found_rust && languages.get(&LanguageType::Rust).is_some() {
            println!("FOUND RUST FOR THE FIRST TIME at commit {}!", &commit.id().to_string()[..8]);
            found_rust = true;
        }
        steps.push(TokeiStepStatistics {
            commit_date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            languages,
        });
    })?;

    Ok(TokeiStatistics { steps })
}

pub struct CargoStepStatistics {
    pub commit_id: String,
    pub commit_date: chrono::DateTime<chrono::Utc>,
    pub num_cargo_toml: usize,
}

pub struct CargoStatistics {
    pub steps: Vec<CargoStepStatistics>,
}

pub fn cargo(owner: &str, repo: &str, temp_dir: &str, num_commits: usize) -> Result<CargoStatistics, AnalyzeError> {
    let mut steps = Vec::new();
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);
    let repo_path = Path::new(&temp_repo_dir);

    analyze_commits(owner, repo, temp_dir, num_commits, &mut |commit, _i| {
        let num_cargo_toml = WalkDir::new(repo_path)
            .into_iter()
            .filter_entry(|e| {
                if let Some(name) = e.file_name().to_str() {
                    name != ".git" && name != "target"
                } else {
                    true
                }
            })
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                if let Some(name) = e.file_name().to_str() {
                    name == "Cargo.toml" || name.ends_with(".cargo.toml")
                } else {
                    false
                }
            })
            .count();

        steps.push(CargoStepStatistics {
            commit_id: commit.id().to_string(),
            commit_date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            num_cargo_toml,
        });
    })?;

    Ok(CargoStatistics { steps })
}

pub const COPY_PHRASES: &[(isize, &str)] = &[
    (3, "rust clone of"), // 52
    (3, "rust copy of"), // 2
    (3, "rust mirror of"), // 2
];

pub const REPLACEMENT_PHRASES: &[(isize, &str)] = &[
    (5, "rust replacement for"), // 51

    (3, "dropin replacement"), // 5.8k
    (3, "drop in replacement"), // 38.7k
    (3, "rust equivalent of"), // 125

    (1, "replacement for"), // 74.2k
    (1, "replacement of"), // 20k
    (1, "successor to"), // 14k
    (1, "the new version of"), // 16k
    (1, "supersedes"), // 7.2k
    (1, "rust alternative to"), // 410
];

pub const DERIVATION_PHRASES: &[(isize, &str)] = &[
    (3, "rust implementation of"), // 10.3k
    (3, "rust version of"), // 1.3k
    (3, "rustbased implementation of"), // 189
    (3, "rust based implementation of"), // 37
    (3, "rust reimplementation of"), // 364
    (3, "reimplemented in rust"), // 107
    
    (1, "is based on"),
    (1, "fork of"),
    (1, "forked from"),
    (1, "derived from"),
    (1, "adapted from"),
    (1, "port of"),
    (1, "ported from"),
    (1, "rust adaptation of"),
    (1, "rust port of"),
    (1, "ported to rust"),

    (0, "based on"),
    (0, "implementation of"),
];

pub const MIGRATION_PHRASES: &[(isize, &str)] = &[
    (5, "rust rewrite of"), // 832
    (5, "rewritten in rust"), // 868
    (5, "rewrite in rust"), // 314
    (5, "rewriting in rust"), // 42
    (5, "converted to rust"), // 142
    (5, "migrated to rust"), // 32

    (3, "now in rust"), // 127

    (1, "migrated to"),
    (1, "converted to"),
    (1, "migration to"),
    (1, "transition to"),
];

pub const COMPATIBILITY_PHRASES: &[(isize, &str)] = &[
    (1, "apicompatible with"),
    (1, "same api as"),
    (1, "plugandplay replacement"),
    (1, "can be used as a dropin"),

    (-5, "rust bindings for"), // 4.1k
    (-5, "rust wrapper for"), // 912
];

pub struct TextStepStatistics {
    pub commit_date: chrono::DateTime<chrono::Utc>,
    pub copy_count: Vec<usize>,
    pub replacement_count: Vec<usize>, 
    pub derivation_count: Vec<usize>,
    pub migration_count: Vec<usize>,
    pub compatibility_count: Vec<usize>,
}

pub struct TextStatistics {
    pub steps: Vec<TextStepStatistics>,
}

fn count_phrases(content: &str, phrases: &[(isize, &str)]) -> Vec<usize> {
    let mut counts = Vec::new();
    for (phrase_index, phrase) in phrases.iter().enumerate() {
        let count = content.matches(phrase.1).count();
        if counts.len() <= phrase_index {
            counts.push(0);
        }
        counts[phrase_index] += count;
    }
    counts
}

pub fn text(owner: &str, repo: &str, temp_dir: &str, num_commits: usize) -> Result<TextStatistics, AnalyzeError> {
    let mut steps = Vec::new();
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);
    let repo_path = Path::new(&temp_repo_dir);

    analyze_commits(owner, repo, temp_dir, num_commits, &mut |commit, _i| {
        let content = match fs::read_to_string(repo_path.join("README.md")) {
            Ok(s) => s,
            Err(_) => return,
        };

        let lower_content = content.to_lowercase().replace("-", "");

        let copy_count = count_phrases(&lower_content, COPY_PHRASES);
        let replacement_count = count_phrases(&lower_content, REPLACEMENT_PHRASES);
        let derivation_count = count_phrases(&lower_content, DERIVATION_PHRASES);
        let migration_count = count_phrases(&lower_content, MIGRATION_PHRASES);
        let compatibility_count = count_phrases(&lower_content, COMPATIBILITY_PHRASES);

        let sum: isize = COPY_PHRASES.iter().zip(copy_count.iter())
            .map(|((score, _), count)| score * (*count as isize))
            .sum::<isize>()
            + REPLACEMENT_PHRASES.iter().zip(replacement_count.iter())
            .map(|((score, _), count)| score * (*count as isize))
            .sum::<isize>()
            + DERIVATION_PHRASES.iter().zip(derivation_count.iter())
            .map(|((score, _), count)| score * (*count as isize))
            .sum::<isize>()
            + MIGRATION_PHRASES.iter().zip(migration_count.iter())
            .map(|((score, _), count)| score * (*count as isize))
            .sum::<isize>()
            + COMPATIBILITY_PHRASES.iter().zip(compatibility_count.iter())
            .map(|((score, _), count)| score * (*count as isize))
            .sum::<isize>();

        println!("Commit {}: sum={}, copy_count={:?}, replacement_count={:?}, derivation_count={:?}, migration_count={:?}, compatibility_count={:?}",
            &commit.id().to_string()[..8],
            sum,
            copy_count,
            replacement_count,
            derivation_count,
            migration_count,
            compatibility_count,
        );

        steps.push(TextStepStatistics {
            commit_date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            copy_count,
            replacement_count,
            derivation_count,
            migration_count,
            compatibility_count,
        });
    })?;

    Ok(TextStatistics { steps })
}

pub struct MatchesStatistics {
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub common_symbols: HashSet<String>,
}

pub fn matches(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    commit1_hash: &str,
    commit2_hash: &str,
) -> Result<MatchesStatistics, AnalyzeError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| AnalyzeError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(owner, repo, &temp_repo_dir)
            .map_err(|_| AnalyzeError::RepositoryCloneError)?;

    let main_branch = repo_info
        .get_main_branch()
        .ok_or(AnalyzeError::MainBranchNotFound)?;

    // Checkout to commit before migration
    let commit1_obj = repo_info.repository
        .revparse_single(commit1_hash)
        .map_err(|_| AnalyzeError::CommitLookupError)?;
    repo_info
        .checkout_commit(commit1_obj.id(), &main_branch)
        .map_err(|_| AnalyzeError::CheckoutError)?;

    let folder = Path::new(&temp_repo_dir);

    let start_time = Instant::now();
    println!("Reading non-Rust symbols at BEFORE point (commit {} in {:?})", commit1_hash, folder);
    let symbols_before_no_rust = code::read_folder_symbols(folder, None, Some(LanguageType::Rust))
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    let symbols_before_no_rust_names = symbols_before_no_rust
        .iter()
        .map(|sym| &sym.name)
        .collect::<HashSet<_>>();
    let before_non_rust_count = symbols_before_no_rust_names.len();
    println!("BEFORE: found {} non-Rust symbols (scanned in {} ms)", before_non_rust_count, start_time.elapsed().as_millis());

    // Checkout to commit after migration
    let commit2_obj = repo_info.repository
        .revparse_single(commit2_hash)
        .map_err(|_| AnalyzeError::CommitLookupError)?;
    repo_info
        .checkout_commit(commit2_obj.id(), &main_branch)
        .map_err(|_| AnalyzeError::CheckoutError)?;

    let start_time = Instant::now();
    println!("Reading Rust symbols at AFTER point (commit {} in {:?})", commit2_hash, folder);
    let symbols_after_rust = code::read_folder_symbols(folder, Some(LanguageType::Rust), None)
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    let symbols_after_rust_names = symbols_after_rust
        .iter()
        .map(|sym| &sym.name)
        .collect::<HashSet<_>>();
    let after_rust_count = symbols_after_rust_names.len();
    println!("AFTER: found {} Rust symbols (scanned in {} ms)", after_rust_count, start_time.elapsed().as_millis());

    let start_time = Instant::now();
    let symbols_after_no_rust = code::read_folder_symbols(folder, None, Some(LanguageType::Rust))
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    let symbols_after_no_rust_names = symbols_after_no_rust
        .iter()
        .map(|sym| &sym.name)
        .collect::<HashSet<_>>();
    let after_non_rust_count = symbols_after_no_rust_names.len();
    println!("AFTER: found {} non-Rust symbols (scanned in {} ms)", after_non_rust_count, start_time.elapsed().as_millis());
    
    let intersection1 = symbols_before_no_rust_names
        .intersection(&symbols_after_rust_names)
        .cloned()
        .collect::<HashSet<_>>();
    let common_count = intersection1.len();

    let pct_rust_that_existed_before = if after_rust_count > 0 {
        (common_count as f64 / after_rust_count as f64) * 100.0
    } else {
        0.0
    };

    let pct_before_non_rust_that_became_rust = if before_non_rust_count > 0 {
        (common_count as f64 / before_non_rust_count as f64) * 100.0
    } else {
        0.0
    };
    
    println!(
        "OVERLAP: {} symbols appear both as non-Rust BEFORE and as Rust AFTER \
        ({:.1}% of all Rust symbols after, {:.1}% of all non-Rust symbols before).",
        common_count,
        pct_rust_that_existed_before,
        pct_before_non_rust_that_became_rust
    );

    let rust_after_unique = intersection1
        .difference(&symbols_after_no_rust_names)
        .cloned()
        .collect::<HashSet<_>>();
    let moved_count = rust_after_unique.len();

    let pct_overlap_that_moved = if common_count > 0 {
        (moved_count as f64 / common_count as f64) * 100.0
    } else {
        0.0
    };

    let pct_rust_symbols_that_moved = if after_rust_count > 0 {
        (moved_count as f64 / after_rust_count as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "MOVED: {} of the {} overlapping symbols are no longer present in non-Rust AFTER \
        ({:.1}% of the overlap, {:.1}% of all Rust symbols after).",
        moved_count,
        common_count,
        pct_overlap_that_moved,
        pct_rust_symbols_that_moved
    );

    println!("  Longest moved symbols:");
    let mut rust_after_unique_vec = rust_after_unique.iter().cloned().collect::<Vec<_>>();
    rust_after_unique_vec.sort_by_key(|s| -(s.len() as isize));
    for sym in rust_after_unique_vec.iter().take(10) {
        println!("    {}", sym);
    }

    Ok(MatchesStatistics {
        common_symbols: intersection1.into_iter().cloned().collect(),
        before: symbols_before_no_rust_names.into_iter().cloned().collect(),
        after: symbols_after_rust_names.into_iter().cloned().collect(),
    })
}

pub struct MatchesStepStatistics {
    pub commit_date: chrono::DateTime<chrono::Utc>,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub common_symbols: HashSet<String>,
    pub common_pre: f64,
    pub common_now: f64,
    pub overlap_that_moved: f64,
    pub moved_now: f64,
}

pub struct Matches2Statistics {
    pub steps: Vec<MatchesStepStatistics>,
}

pub fn matches2(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    commit_hash: &str,
) -> Result<Matches2Statistics, AnalyzeError> {
    let mut steps = Vec::new();
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(owner, repo, &temp_repo_dir)
            .map_err(|_| AnalyzeError::RepositoryCloneError)?;

    let main_branch = repo_info
        .get_main_branch()
        .ok_or(AnalyzeError::MainBranchNotFound)?;

    let commits = repo_info
        .get_commits(&main_branch)
        .map_err(|_| AnalyzeError::RevwalkError)?;

    let commit_count = commits.len();
    if commit_count == 0 {
        return Ok(Matches2Statistics { steps });
    }

    let before_commit = repo_info.repository
        .revparse_single(commit_hash)
        .map_err(|_| AnalyzeError::CommitLookupError)?;
    repo_info
        .checkout_commit(before_commit.id(), &main_branch)
        .map_err(|_| AnalyzeError::CheckoutError)?;

    let folder = Path::new(&temp_repo_dir);

    let symbols_before_no_rust = code::read_folder_symbols(folder, None, Some(LanguageType::Rust))
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    let symbols_before_no_rust_names = symbols_before_no_rust
        .iter()
        .map(|sym| &sym.name)
        .collect::<HashSet<_>>();

    let indices = sample_indices(commit_count, 100).into_iter().rev().collect::<Vec<_>>();
    let mut has_passed_known_commit = false;

    for commit_index in indices {
        let commit_id = commits[commit_index];
        if commit_id == before_commit.id() {
            has_passed_known_commit = true;
        } else if !has_passed_known_commit {
            continue;
        }
        let commit = repo_info
            .repository
            .find_commit(commit_id)
            .map_err(|_| AnalyzeError::CommitLookupError)?;

        repo_info
            .checkout_commit(commit_id, &main_branch)
            .map_err(|_| AnalyzeError::CheckoutError)?;

        let symbols_after_rust = code::read_folder_symbols(folder, Some(LanguageType::Rust), None)
            .map_err(|_| AnalyzeError::TempDirCreationError)?;
        let symbols_after_rust_names = symbols_after_rust
            .iter()
            .map(|sym| &sym.name)
            .collect::<HashSet<_>>();

        let symbols_after_no_rust = code::read_folder_symbols(folder, None, Some(LanguageType::Rust))
            .map_err(|_| AnalyzeError::TempDirCreationError)?;
        let symbols_after_no_rust_names = symbols_after_no_rust
            .iter()
            .map(|sym| &sym.name)
            .collect::<HashSet<_>>();

        let cloned_symbols_before_no_rust = symbols_before_no_rust_names.clone();

        let intersection = cloned_symbols_before_no_rust
            .intersection(&symbols_after_rust_names)
            .cloned()
            .collect::<HashSet<_>>();
        let common_count = intersection.len();

        let pct_common_pre = if symbols_before_no_rust_names.len() > 0 {
            (common_count as f64 / symbols_before_no_rust_names.len() as f64) * 100.0
        } else {
            0.0
        };

        let pct_common_now = if symbols_after_rust_names.len() > 0 {
            (common_count as f64 / symbols_after_rust_names.len() as f64) * 100.0
        } else {
            0.0
        };

        let rust_after_unique = intersection
            .difference(&symbols_after_no_rust_names)
            .cloned()
            .collect::<HashSet<_>>();
        let moved_count = rust_after_unique.len();

        let pct_moved_overlap = if common_count > 0 {
            (moved_count as f64 / common_count as f64) * 100.0
        } else {
            0.0
        };

        let pct_moved = if symbols_after_rust_names.len() > 0 {
            (moved_count as f64 / symbols_after_rust_names.len() as f64) * 100.0
        } else {
            0.0
        };

        steps.push(MatchesStepStatistics {
            before: cloned_symbols_before_no_rust.into_iter().cloned().collect(),
            after: symbols_after_rust_names.into_iter().cloned().collect(),
            common_symbols: intersection.into_iter().cloned().collect(),
            common_pre: pct_common_pre,
            common_now: pct_common_now,
            overlap_that_moved: pct_moved_overlap,
            moved_now: pct_moved,
            commit_date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
        });

        println!(
            "[{}/{}] Commit {} at {}: common_pre={:.1}%, common_now={:.1}%, moved={:.1}%",
            commit_count - commit_index,
            commit_count,
            &commit.id().to_string()[..8],
            chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            pct_common_pre,
            pct_common_now,
            pct_moved
        );
    }

    Ok(Matches2Statistics { steps })
}

pub fn overlap(
    owner1: &str,
    repo1: &str,
    owner2: &str,
    repo2: &str,
    temp_dir: &str,
) -> Result<(), AnalyzeError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| AnalyzeError::TempDirCreationError)?;
    }

    let temp_repo_dir1 = format!("{}/{}_{}", temp_dir, owner1, repo1);
    let repo_info1 =
        repository::BareRepositoryInfo::clone_or_open(owner1, repo1, &temp_repo_dir1)
            .map_err(|_| AnalyzeError::RepositoryCloneError)?;
    let main_branch1 = repo_info1
        .get_main_branch()
        .ok_or(AnalyzeError::MainBranchNotFound)?;

    let temp_repo_dir2 = format!("{}/{}_{}", temp_dir, owner2, repo2);
    let repo_info2 =
        repository::BareRepositoryInfo::clone_or_open(owner2, repo2, &temp_repo_dir2)
            .map_err(|_| AnalyzeError::RepositoryCloneError)?;
    let main_branch2 = repo_info2
        .get_main_branch()
        .ok_or(AnalyzeError::MainBranchNotFound)?;

    let commits1 = repo_info1
        .get_commits(&main_branch1)
        .map_err(|_| AnalyzeError::RevwalkError)?;
    if commits1.is_empty() {
        return Err(AnalyzeError::CommitLookupError);
    }
    let last_commit1 = commits1[0];
    repo_info1
        .checkout_commit(last_commit1, &main_branch1)
        .map_err(|_| AnalyzeError::CheckoutError)?;

    let folder1 = Path::new(&temp_repo_dir1);
    let start_time = Instant::now();
    println!("Reading symbols from REPO1: {}/{} at branch {} in {:?}", owner1, repo1, main_branch1, folder1);
    let symbols_repo1 = code::read_folder_symbols(folder1, None, None)
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    let symbols_repo1_names = symbols_repo1
        .iter()
        .map(|sym| &sym.name)
        .collect::<HashSet<_>>();
    let symbols_repo1_count = symbols_repo1_names.len();
    println!("REPO1: found {} symbols (scanned in {} ms)", symbols_repo1_count, start_time.elapsed().as_millis());

    let commits2 = repo_info2
        .get_commits(&main_branch2)
        .map_err(|_| AnalyzeError::RevwalkError)?;
    if commits2.is_empty() {
        return Err(AnalyzeError::CommitLookupError);
    }
    let last_commit2 = commits2[0];
    repo_info2
        .checkout_commit(last_commit2, &main_branch2)
        .map_err(|_| AnalyzeError::CheckoutError)?;

    let folder2 = Path::new(&temp_repo_dir2);
    let start_time = Instant::now();
    println!("Reading symbols from REPO2: {}/{} at branch {} in {:?}", owner2, repo2, main_branch2, folder2);
    let symbols_repo2 = code::read_folder_symbols(folder2, None, None)
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    let symbols_repo2_names = symbols_repo2
        .iter()
        .map(|sym| &sym.name)
        .collect::<HashSet<_>>();
    let symbols_repo2_count = symbols_repo2_names.len();
    println!("REPO2: found {} symbols (scanned in {} ms)", symbols_repo2_count, start_time.elapsed().as_millis());

    let intersection = symbols_repo1_names
        .intersection(&symbols_repo2_names)
        .cloned()
        .collect::<HashSet<_>>();
    let common_count = intersection.len();
    println!(
        "OVERLAP: {} symbols appear in both repositories \
        ({:.1}% of REPO1, {:.1}% of REPO2).",
        common_count,
        if symbols_repo1_count > 0 {
            (common_count as f64 / symbols_repo1_count as f64) * 100.0
        } else {
            0.0
        },
        if symbols_repo2_count > 0 {
            (common_count as f64 / symbols_repo2_count as f64) * 100.0
        } else {
            0.0
        },
    );

    println!("  Longest common symbols:");
    let mut intersection_vec = intersection.iter().cloned().collect::<Vec<_>>();
    intersection_vec.sort_by_key(|s| -(s.len() as isize));
    for sym in intersection_vec.iter().take(10) {
        println!("    {}", sym);
    }

    Ok(())
}

pub struct CommandStepStatistics {
    pub commit_date: chrono::DateTime<chrono::Utc>,
    pub command_counts: std::collections::HashMap<String, usize>,
}

pub struct CommandStatistics {
    pub steps: Vec<CommandStepStatistics>,
}

pub fn commands(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    num_commits: usize,
) -> Result<CommandStatistics, AnalyzeError> {
    let mut steps = Vec::new();
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);
    let repo_path = Path::new(&temp_repo_dir);

    analyze_commits(owner, repo, temp_dir, num_commits, &mut |commit, _i| {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for entry in WalkDir::new(repo_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            // only consider markdown files
            if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                if !ext.eq_ignore_ascii_case("md") {
                    continue;
                }
            } else {
                continue;
            }

            let content = match fs::read_to_string(entry.path()) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut in_code_block = false;
            for raw_line in content.lines() {
                let line = raw_line.trim_start();

                if line.starts_with("```shell") || line.starts_with("```bash") || line.starts_with("```sh") {
                    in_code_block = true;
                    continue;
                }
                else if line.starts_with("```") {
                    in_code_block = false;
                    continue;
                }

                if !in_code_block {
                    continue;
                }

                let normalized = line.replace("&&", ";").replace("||", ";");
                for segment in normalized.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let segment = segment.trim_start_matches(|c: char| c == '$' || c == '>' || c == '#').trim();

                    if segment.is_empty() {
                        continue;
                    }

                    for part in segment.split('|').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        let mut tokens = part.split_whitespace().filter(|t| !t.is_empty());
                        let mut cmd_token: Option<&str> = None;
                        while let Some(tok) = tokens.next() {
                            let lower = tok.to_ascii_lowercase();
                            if lower == "export" {
                                continue;
                            }
                            if tok.contains('=') {
                                continue;
                            }
                            if tok.starts_with(&['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z']) {
                                continue;
                            }
                            cmd_token = Some(tok);
                            break;
                        }

                        if let Some(cmd) = cmd_token {
                            let entry = counts.entry(cmd.to_string()).or_insert(0);
                            *entry += 1;
                        }
                    }
                }
            }
        }

        steps.push(CommandStepStatistics {
            commit_date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            command_counts: counts,
        });
    })?;

    Ok(CommandStatistics { steps })
}

pub struct MigrationStatus {
    pub min_rust: f64,
    pub max_rust: f64,
    pub peak_moved: f64,
}

pub fn determine_status(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    num_commits: usize,
) -> Result<MigrationStatus, AnalyzeError> {
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);
    let folder = Path::new(&temp_repo_dir);
    let paths = [temp_repo_dir.as_str()];
    let excluded: [&str; 0] = [];
    let config = Config::default();
    let mut rust_symbols = Vec::new();
    let mut non_rust_symbols = Vec::new();

    let mut found_rust: isize = -1;
    let mut peak_moved = 0.0;
    let mut min_rust = 100.0;
    let mut max_rust = 0.0;

    analyze_commits(owner, repo, temp_dir, num_commits, &mut |commit, i| {
        let start_time = Instant::now();
        let mut languages = Languages::new();
        languages.get_statistics(&paths, &excluded, &config);
        non_rust_symbols.push(code::read_folder_symbols(folder, None, Some(LanguageType::Rust))
        .unwrap());
        rust_symbols.push(code::read_folder_symbols(folder, Some(LanguageType::Rust), None)
        .unwrap());

        let total_loc = languages.total().code as f64;
        let rust_loc = if let Some(rust_lang) = languages.get(&LanguageType::Rust) {
            rust_lang.code as f64
        } else {
            0.0
        };
        let rust_pct = if total_loc > 0.0 {
            rust_loc / total_loc
        } else {
            0.0
        };
        if rust_pct < min_rust {
            min_rust = rust_pct;
        }
        if rust_pct > max_rust {
            max_rust = rust_pct;
        }
        if found_rust < 0 && rust_loc > 0.0 {
            println!("First rust at commit {}!", &commit.id().to_string()[..8]);
            found_rust = i as isize;
        }
        if found_rust > 0 {
            let before_non_rust_symbols = &non_rust_symbols[(found_rust - 1) as usize];
            let before_non_rust_names = before_non_rust_symbols
                .iter()
                .map(|sym| &sym.name)
                .collect::<HashSet<_>>();
            let after_rust_symbols = &rust_symbols[i];
            let after_rust_names = after_rust_symbols
                .iter()
                .map(|sym| &sym.name)
                .collect::<HashSet<_>>();
            let intersection = before_non_rust_names
                .intersection(&after_rust_names)
                .cloned()
                .collect::<HashSet<_>>();
            let common_count = intersection.len();

            let after_non_rust_symbols = &non_rust_symbols[i];
            let after_non_rust_names = after_non_rust_symbols
                .iter()
                .map(|sym| &sym.name)
                .collect::<HashSet<_>>();
            let rust_after_unique = intersection
                .difference(&after_non_rust_names)
                .cloned()
                .collect::<HashSet<_>>();
            let moved_count = rust_after_unique.len();

            let pct_moved = if common_count > 0 {
                (moved_count as f64 / common_count as f64)
            } else {
                0.0
            };
            if pct_moved > peak_moved {
                peak_moved = pct_moved;
            }

            println!(
                "[{}/{}] Commit {} at {}: rust_pct={:.1}%, moved={:.1}%, common={} in {} ms",
                i + 1,
                num_commits,
                &commit.id().to_string()[..8],
                chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
                rust_pct * 100.0,
                pct_moved * 100.0,
                common_count,
                start_time.elapsed().as_millis()
            );
        }
    })?;

    Ok(MigrationStatus { peak_moved, min_rust, max_rust } )
}