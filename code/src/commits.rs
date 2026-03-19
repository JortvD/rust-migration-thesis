use std::fs;
use serde::Serialize;
use git2::{Oid, Repository};

#[derive(Debug, Clone, Serialize)]
struct FileChange {
    old_path: Option<String>,
    new_path: Option<String>,
    status: String,
    old_is_binary: bool,
    new_is_binary: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CommitStats {
    hash: String,
    tree_hash: String,
    parents: Vec<String>,
    is_merge: bool,

    subject: String,
    description: String,
    message_encoding: Option<String>,

    author_name: String,
    author_email: String,
    author_date: i64,
    author_tz_offset: i32,

    committer_name: String,
    committer_email: String,
    committer_date: i64,
    committer_tz_offset: i32,

    is_signed: bool,

    files_changed: usize,
    lines_added: usize,
    lines_deleted: usize,

    file_details: Vec<FileChange>,
}

pub fn run(commits: &Vec<Oid>, repo: &Repository, result_folder: &str) {
    let stats: Vec<CommitStats> = commits.into_iter().filter_map(|oid| {
        let commit = repo.find_commit(*oid).ok()?;

        let author = commit.author();
        let committer = commit.committer();

        let tree = commit.tree().ok()?;
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = repo.diff_tree_to_tree(
            parent_tree.as_ref(),
            Some(&tree),
            None
        ).ok()?;

        let diff_stats = diff.stats().ok()?;

        let file_details: Vec<FileChange> = diff.deltas().map(|delta| {
            FileChange {
                old_path: delta.old_file().path().map(|p| p.to_string_lossy().into_owned()),
                new_path: delta.new_file().path().map(|p| p.to_string_lossy().into_owned()),
                status: format!("{:?}", delta.status()),
                old_is_binary: delta.old_file().is_binary(),
                new_is_binary: delta.new_file().is_binary(),
            }
        }).collect();

        let is_signed = repo.extract_signature(&oid, None).is_ok();

        Some(CommitStats {
            hash: commit.id().to_string(),
            tree_hash: commit.tree_id().to_string(),
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
            is_merge: commit.parent_count() > 1,
            
            subject: commit.summary().unwrap_or("").to_string(),
            description: commit.body().unwrap_or("").to_string(),
            message_encoding: commit.message_encoding().map(|s| s.to_string()),
            
            author_name: author.name().unwrap_or("").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            author_date: author.when().seconds(),
            author_tz_offset: author.when().offset_minutes(),
            
            committer_name: committer.name().unwrap_or("").to_string(),
            committer_email: committer.email().unwrap_or("").to_string(),
            committer_date: committer.when().seconds(),
            committer_tz_offset: committer.when().offset_minutes(),
            
            is_signed,
            
            files_changed: diff_stats.files_changed(),
            lines_added: diff_stats.insertions(),
            lines_deleted: diff_stats.deletions(),
            
            file_details,
        })
    }).collect();
    
    let out_path = format!("{}/commit_stats.json", result_folder);
    fs::write(out_path, serde_json::to_string_pretty(&stats).unwrap())
        .expect("Failed to write output");
}