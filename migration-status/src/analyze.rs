use std::fs;
use std::path::Path;
use std::time::Instant;

use git2::build::CheckoutBuilder;
use tokei::{Config, LanguageType, Languages};
use chrono;
use walkdir::WalkDir;

use crate::repository;

#[derive(Debug)]
pub enum AnalyzeError {
    TempDirCreationError,
    RepositoryCloneError,
    MainBranchNotFound,
    RevwalkError,
    TreeLookupError,
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

pub fn is_code_language(lang: &LanguageType) -> bool {
    match lang {
        LanguageType::ActionScript => true,
        LanguageType::Ada => true,
        LanguageType::Agda => true,
        LanguageType::Alloy => true,
        LanguageType::Arduino => true,
        LanguageType::Asp => true,
        LanguageType::AspNet => true,
        LanguageType::Assembly => true,
        LanguageType::AssemblyGAS => true,
        LanguageType::AutoHotKey => true,
        LanguageType::Bash => true,
        LanguageType::Batch => true,
        LanguageType::BrightScript => true,
        LanguageType::C => true,
        LanguageType::CSharp => true,
        LanguageType::CShell => true,
        LanguageType::Clojure => true,
        LanguageType::ClojureC => true,
        LanguageType::ClojureScript => true,
        LanguageType::Cobol => true,
        LanguageType::CodeQL => true,
        LanguageType::CoffeeScript => true,
        LanguageType::Coq => true,
        LanguageType::Cpp => true,
        LanguageType::Crystal => true,
        LanguageType::D => true,
        LanguageType::Dart => true,
        LanguageType::DeviceTree => true,
        LanguageType::Elisp => true,
        LanguageType::Elixir => true,
        LanguageType::Elm => true,
        LanguageType::Elvish => true,
        LanguageType::Emojicode => true,
        LanguageType::Erlang => true,
        LanguageType::FSharp => true,
        LanguageType::Forth => true,
        LanguageType::FortranLegacy => true,
        LanguageType::FortranModern => true,
        LanguageType::Fstar => true,
        LanguageType::Futhark => true,
        LanguageType::GdScript => true,
        LanguageType::Gleam => true,
        LanguageType::Glsl => true,
        LanguageType::Go => true,
        LanguageType::Groovy => true,
        LanguageType::Gwion => true,
        LanguageType::Haskell => true,
        LanguageType::Haxe => true,
        LanguageType::Hlsl => true,
        LanguageType::HolyC => true,
        LanguageType::Idris => true,
        LanguageType::Isabelle => true,
        LanguageType::Jai => true,
        LanguageType::Java => true,
        LanguageType::JavaScript => true,
        LanguageType::Jsonnet => true,
        LanguageType::Jsx => true,
        LanguageType::Julia => true,
        LanguageType::Julius => true,
        LanguageType::K => true,
        LanguageType::KakouneScript => true,
        LanguageType::Kotlin => true,
        LanguageType::Lean => true,
        LanguageType::Lisp => true,
        LanguageType::LiveScript => true,
        LanguageType::Logtalk => true,
        LanguageType::Lua => true,
        LanguageType::Madlang => true,
        LanguageType::MoonScript => true,
        LanguageType::Nim => true,
        LanguageType::NotQuitePerl => true,
        LanguageType::OCaml => true,
        LanguageType::ObjectiveC => true,
        LanguageType::ObjectiveCpp => true,
        LanguageType::Odin => true,
        LanguageType::Oz => true,
        LanguageType::PSL => true,
        LanguageType::Pascal => true,
        LanguageType::Perl => true,
        LanguageType::Perl6 => true,
        LanguageType::Php => true,
        LanguageType::Pony => true,
        LanguageType::Processing => true,
        LanguageType::Prolog => true,
        LanguageType::PureScript => true,
        LanguageType::Python => true,
        LanguageType::Q => true,
        LanguageType::Qcl => true,
        LanguageType::Qml => true,
        LanguageType::R => true,
        LanguageType::Racket => true,
        LanguageType::Renpy => true,
        LanguageType::Ruby => true,
        LanguageType::Rust => true,
        LanguageType::Scala => true,
        LanguageType::Scheme => true,
        LanguageType::Sml => true,
        LanguageType::Solidity => true,
        LanguageType::SpecmanE => true,
        LanguageType::Spice => true,
        LanguageType::Sql => true,
        LanguageType::Stan => true,
        LanguageType::Stratego => true,
        LanguageType::Svelte => true,
        LanguageType::Swift => true,
        LanguageType::Swig => true,
        LanguageType::SystemVerilog => true,
        LanguageType::Tcl => true,
        LanguageType::Tsx => true,
        LanguageType::TypeScript => true,
        LanguageType::UnrealScript => true,
        LanguageType::Vala => true,
        LanguageType::Verilog => true,
        LanguageType::Vhdl => true,
        LanguageType::VimScript => true,
        LanguageType::VisualBasic => true,
        LanguageType::VB6 => true,
        LanguageType::VBScript => true,
        LanguageType::WebAssembly => true,
        LanguageType::Wolfram => true,
        LanguageType::Xtend => true,
        LanguageType::Zig => true,
        LanguageType::Zsh => true,
        _ => false,
    }
}

fn analyze_commits(owner: &str, repo: &str, temp_dir: &str, num_commits: usize, func: &mut dyn FnMut(&git2::Commit) ) -> Result<(), AnalyzeError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| AnalyzeError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::RepositoryInfo::clone_or_open(owner, repo, &temp_repo_dir)
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

    for commit_index in indices {
        let commit_id = commits[commit_index];
        let start_time = Instant::now();

        let commit = repo_info
            .checkout_commit(commit_id, &main_branch)
            .map_err(|_| AnalyzeError::CheckoutError)?;

        func(&commit);

        println!(
            "[{}/{}] Commit {} at {} in {} ms",
            commit_count - commit_index,
            commit_count,
            &commit.id().to_string()[..8],
            chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            start_time.elapsed().as_millis()
        );
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

    analyze_commits(owner, repo, temp_dir, num_commits, &mut |commit| {
        let mut languages = Languages::new();
        languages.get_statistics(&paths, &excluded, &config);
        steps.push(TokeiStepStatistics {
            commit_date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            languages,
        });
    })?;

    Ok(TokeiStatistics { steps })
}

pub struct CargoStepStatistics {
    pub commit_date: chrono::DateTime<chrono::Utc>,
    pub num_toml_files: usize,
}

pub struct CargoStatistics {
    pub steps: Vec<CargoStepStatistics>,
}

pub fn cargo(owner: &str, repo: &str, temp_dir: &str, num_commits: usize) -> Result<CargoStatistics, AnalyzeError> {
    let mut steps = Vec::new();
    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);
    let repo_path = Path::new(&temp_repo_dir);

    analyze_commits(owner, repo, temp_dir, num_commits, &mut |commit| {
        let num_toml_files = WalkDir::new(repo_path)
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
            commit_date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(commit.time().seconds()).expect("Error"),
            num_toml_files,
        });
    })?;

    Ok(CargoStatistics { steps })
}

use std::collections::HashSet;
use std::io::Read;
use std::path::PathBuf;

use bio::data_structures::suffix_array::{lcp, suffix_array};

const MIN_LEN: usize = 5;

fn read_folder_text(root: &Path, match_language: Option<LanguageType>, not_match_language: Option<LanguageType>) -> Result<String, Box<dyn std::error::Error>> {
    let mut text = String::new();

    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
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
        let language = LanguageType::from_file_extension(
            path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
        );

        if let Some(lang) = language {
            if !is_code_language(&lang) {
                continue;
            }

            if let Some(match_lang) = match_language {
                if lang != match_lang {
                    continue;
                }
            }

            if let Some(not_match_lang) = not_match_language {
                if lang == not_match_lang {
                    continue;
                }
            }
        } else {
            continue;
        }

        println!("Reading file: {:?}", path);

        let mut file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            continue;
        }

        text.push_str(&buf);
        text.push('\n');
    }

    Ok(text)
}

fn common_substrings(a: &str, b: &str, min_len: usize) -> HashSet<String> {
    const SEP: u8 = 1;
    const SENTINEL: u8 = 0;

    let mut combined: Vec<u8> = Vec::with_capacity(a.len() + b.len() + 2);
    combined.extend_from_slice(a.as_bytes());
    let sep_index = combined.len();
    combined.push(SEP);
    combined.extend_from_slice(b.as_bytes());
    combined.push(SENTINEL);
    let sentinel_index = combined.len() - 1;

    let sa = suffix_array(&combined);
    let lcp_arr = lcp(&combined, &sa);

    let mut result = HashSet::new();

    for i in 1..sa.len() {
        let pos1 = sa[i - 1];
        let pos2 = sa[i];

        if pos1 == sep_index || pos1 == sentinel_index || pos2 == sep_index || pos2 == sentinel_index {
            continue;
        }

        let in_a1 = pos1 < sep_index;
        let in_a2 = pos2 < sep_index;

        if in_a1 == in_a2 {
            continue;
        }

        let lcp_val: isize = lcp_arr.get(i).unwrap_or(0);
        if lcp_val <= 0 {
            continue;
        }

        let mut l = lcp_val as usize;

        if pos1 + l > sentinel_index {
            l = sentinel_index - pos1;
        }

        if pos1 < sep_index && pos1 + l > sep_index {
            l = sep_index - pos1;
        }

        if l < min_len {
            continue;
        }

        let start = pos1;
        let end = start + l;

        if let Ok(sub) = std::str::from_utf8(&combined[start..end]) {
            result.insert(sub.to_string());
        }
    }

    result
}

pub fn matches(
    owner: &str,
    repo: &str,
    temp_dir: &str,
    commit1_hash: &str,
    commit2_hash: &str,
) -> Result<(), AnalyzeError> {
    if !Path::new(temp_dir).exists() {
        fs::create_dir_all(temp_dir).map_err(|_| AnalyzeError::TempDirCreationError)?;
    }

    let temp_repo_dir = format!("{}/{}_{}", temp_dir, owner, repo);

    let repo_info =
        repository::RepositoryInfo::clone_or_open(owner, repo, &temp_repo_dir)
            .map_err(|_| AnalyzeError::RepositoryCloneError)?;

    let main_branch = repo_info
        .get_main_branch()
        .ok_or(AnalyzeError::MainBranchNotFound)?;

    let commit1_obj = repo_info.repository
        .revparse_single(commit1_hash)
        .map_err(|_| AnalyzeError::CommitLookupError)?;
    let commit1 = repo_info
        .checkout_commit(commit1_obj.id(), &main_branch)
        .expect("");

    let folder = Path::new(&temp_repo_dir);

    println!("Reading folder for commit {}: {:?}", commit1_hash, folder);
    let text1 = read_folder_text(folder, None, Some(LanguageType::Rust))
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    println!("Total bytes in folder 1 text: {}", text1.len());

    let commit2_obj = repo_info.repository
        .revparse_single(commit2_hash)
        .map_err(|_| AnalyzeError::CommitLookupError)?;
    let commit2 = repo_info
        .checkout_commit(commit2_obj.id(), &main_branch)
        .expect("");

    println!("Reading folder for commit {}: {:?}", commit2_hash, folder);
    let text2 = read_folder_text(folder, Some(LanguageType::Rust), None)
        .map_err(|_| AnalyzeError::TempDirCreationError)?;
    println!("Total bytes in folder 2 text: {}", text2.len());

    println!(
        "Finding common substrings of length >= {}...",
        MIN_LEN
    );
    let common = common_substrings(&text1, &text2, MIN_LEN);

    println!(
        "Found {} distinct common substrings of length >= {}.",
        common.len(),
        MIN_LEN
    );

    // Print them (or write to a file if it’s too many)
    for s in &common {
        println!("{s}");
    }
    Ok(())
}
