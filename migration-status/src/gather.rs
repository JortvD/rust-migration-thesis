use core::num;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tokei::{Config, LanguageType, Languages};
use chrono;
use walkdir::WalkDir;
use std::collections::{HashMap, HashSet};
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


pub const PHRASES: &[&str] = &[
    "rust clone of",
    "rust copy of",
    "rust mirror of",
    "rust replacement for",
    "rust implementation of",
    "rust version of",
    "rust rewrite of",
    "rewritten in rust",
    "now in rust",
    "rust alternative to",
    "rust based implementation of",
    "rustbased implementation of",
    "rust reimplementation of",
    "reimplemented in rust",
    "rust adaptation of",
    "converted to rust",
    "adapted to rust",
    "migrated to rust",
    "transitioned to rust",
    "rewriting to rust",
    "migration to rust",
];

pub struct TextMatch {
    pub phrase: String,
    pub before: String,
    pub after: String,
}

const MATCH_CONTEXT_CHARS: usize = 30;

fn find_text_matches(path: &Path) -> Vec<TextMatch> {
    let mut matches = Vec::new();
    
    for entry in WalkDir::new(path).into_iter().filter_entry(|e| {
        if let Some(name) = e.file_name().to_str() {
            name != ".git" && name != "target"
        } else {
            true
        }
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path: PathBuf = entry.path().into();

        if path.extension().is_none() || path.extension().unwrap().to_str() != Some("md") {
            continue;
        }

        let mut text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        text = text.replace("-", "").replace("\n", " ").replace("\r", " ").to_lowercase();

        for phrase in PHRASES {
            let mut start = 0;
            while let Some(pos) = text[start..].find(phrase) {
                let match_start = start + pos;
                let match_end = match_start + phrase.len();

                let before_start = match match_start.checked_sub(MATCH_CONTEXT_CHARS) {
                    Some(v) => v,
                    None => 0,
                };
                let before = &text[before_start..match_start];

                let after_end = (match_end + MATCH_CONTEXT_CHARS).min(text.len());
                let after = &text[match_end..after_end];

                start = match_end;
            }
        }
    }
    
    matches
    
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
    writer: &mut dyn std::io::Write,
    func: &mut dyn FnMut(&git2::Commit, usize, &String, &mut dyn std::io::Write) -> bool,
)-> Result<(), GatherError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| GatherError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::BareRepositoryInfo::clone_or_open(writer, owner, repo, &temp_repo_dir)
            .map_err(|_| GatherError::RepositoryCloneError)?;

    let main_branch = repo_info
        .get_main_branch()
        .ok_or(GatherError::MainBranchNotFound)?;

    writeln!(
        writer,
        "Analyzing branch: {main_branch}"
    ).expect("Failed to write to writer");
    let start_time = Instant::now();

    let commits = repo_info
        .get_commits(&main_branch)
        .map_err(|_| GatherError::RevwalkError)?;

    let commit_count = commits.len();
    writeln!(
        writer,
        "Commits in branch {main_branch}: {} (in {} ms)",
        commit_count,
        start_time.elapsed().as_millis()
    ).expect("Failed to write to writer");

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

        writeln!(
            writer,
            "[{}][{}] Checked out commit {} at {} (took {} ms)",
            i,
            chrono::Utc::now().to_rfc3339(),
            &commit.id().to_string()[..8],
            chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            start_time.elapsed().as_millis(),
        ).expect("Failed to write to writer");

        let result = func(&commit, i, &temp_repo_dir, writer);

        if !result {
            break;
        }
    }

    Ok(())
}

pub struct AnalyzeResult {
    pub lang_stats: HashMap<LanguageType, (f64, usize)>,
    pub symbols: HashMap<code::SupportedLanguage, HashSet<String>>,
    pub text_matches: Vec<TextMatch>,
    pub can_continue: bool,
}

pub fn analyze_commit(
    repo: &String,
    commit: &git2::Commit,
    index: usize,
    dir: &String,
    writer: &mut dyn std::io::Write,
) -> AnalyzeResult {
    let path = Path::new(dir);
    let paths = [dir.as_str()];
    let excluded: [&str; 0] = [];
    let config = Config::default();

    let start_time = Instant::now();
    let (symbols, file_count) = code::find_symbols(path).unwrap_or_default();
    let measured_symbols_count: usize = symbols.values().map(|s| s.len()).sum();
    let symbol_measure_duration = start_time.elapsed().as_millis();

    let start_time = Instant::now();
    let mut languages = Languages::new();
    languages.get_statistics(&paths, &excluded, &config);
    let mut lang_stats = HashMap::new();
    let total_loc = languages.total().code as f64;
    for (lang, stats) in languages.iter() {
        let loc = stats.code;
        let loc_pct = loc as f64 / total_loc.max(1.0);
        lang_stats.insert(*lang, (loc_pct, loc));
    }
    let lang_measure_duration = start_time.elapsed().as_millis();

    let start_time = Instant::now();
    let text_matches = find_text_matches(path);
    for text_match in &text_matches {
        writeln!(
            writer,
            "[{}][{}] Found text match: ...{}[{}]{}...",
            index,
            chrono::Utc::now().to_rfc3339(),
            text_match.before,
            text_match.phrase,
            text_match.after,
        ).expect("Failed to write to writer");
    }
    let text_match_duration = start_time.elapsed().as_millis();

    writeln!(
        writer,
        "[{}][{}] Analyzed. Language analysis took found {} LOC in {} ms; symbol analysis found {} symbols in {} files in {} ms. Text match analysis found {} matches in {} ms.",
        index,
        chrono::Utc::now().to_rfc3339(),
        total_loc as usize,
        lang_measure_duration,
        measured_symbols_count,
        file_count,
        symbol_measure_duration,
        text_matches.len(),
        text_match_duration,
    ).expect("Failed to write to writer");

    AnalyzeResult {
        lang_stats,
        symbols,
        text_matches,
        can_continue: true,
    }
}


pub struct RepositoryStats {
    pub length: usize,
    pub name: String,
    pub lang_stats: Vec<HashMap<LanguageType, (f64, usize)>>,
    pub symbols: Vec<HashMap<code::SupportedLanguage, HashSet<String>>>,
    pub text_matches: Vec<Vec<TextMatch>>,
}

pub fn gather_repository_statistics(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    num_commits: usize,
    writer: &mut dyn std::io::Write,
) -> Result<RepositoryStats, GatherError> {
    let repo_name = format!("{}/{}", owner, repo);
    let mut symbols = Vec::new();
    let mut lang_stats = Vec::new();
    let mut text_matches = Vec::new();

    select_evenly_spread_commits_and_checkout_each(owner, repo, temp_dir, num_commits, writer, &mut |commit, i, dir, writer| {
        let analyze_result = analyze_commit(&repo_name, commit, i, &dir, writer);
        symbols.push(analyze_result.symbols);
        lang_stats.push(analyze_result.lang_stats);
        text_matches.push(analyze_result.text_matches);
        
        true
    })?;

    Ok(RepositoryStats {
        length: lang_stats.len(),
        name: repo_name,
        lang_stats,
        symbols,
        text_matches,
    })
}