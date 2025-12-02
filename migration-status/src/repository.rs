use git2::{Repository, build::RepoBuilder};
use std::time::Instant;
use std::io::{self, Cursor, Write};

use crate::consts::DEBUG;

pub struct BareRepositoryInfo {
    pub dir: String,
    pub repository: Repository,
}

impl BareRepositoryInfo {
    pub fn clone_or_open(owner: &str, repo: &str, dir: &str) -> Result<Self, git2::Error> {       
        let repo_url = format!("https://github.com/{}/{}.git", owner, repo);
        let git_dir = dir.to_string() + ".git";
        let git_path = std::path::Path::new(&git_dir);
        let repository = if !git_path.exists() {
            match RepoBuilder::new().bare(true).clone(&repo_url, &git_path) {
                Ok(repo) => repo,
                Err(e) => return Err(e),
            }
        } else {
            match Repository::open_bare(&git_dir) {
                Ok(repo) => repo,
                Err(e) => return Err(e),
            }
        };
        Ok(BareRepositoryInfo {
            repository,
            dir: dir.to_string(),
        })
    }

    pub fn get_main_branch(&self) -> Option<String> {
        if self.repository.find_branch("main", git2::BranchType::Local).is_ok() {
            Some("refs/heads/main".to_string())
        } else if self.repository.find_branch("master", git2::BranchType::Local).is_ok() {
            Some("refs/heads/master".to_string())
        } else if self.repository.find_branch("canary", git2::BranchType::Local).is_ok() {
            Some("refs/heads/canary".to_string())
        } else {
            match self.repository.head() {
                Ok(h) => {
                    if let Some(name) = h.name() {
                        Some(name.to_string())
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
    }

    pub fn get_commits(&self, branch_ref: &str) -> Result<Vec<git2::Oid>, git2::Error> {
        let reference = self.repository.find_reference(&branch_ref)?;
        let mut oid = reference
            .target()
            .ok_or_else(|| git2::Error::from_str("Invalid reference target"))?;
        
        let mut commits = Vec::new();

        loop {
            let commit = self.repository.find_commit(oid)?;
            commits.push(oid);

            if commit.parent_count() == 0 {
                break;
            }

            oid = commit.parent_id(0)?;
        }
        Ok(commits)
    }

    pub fn checkout_commit(&self, oid: git2::Oid, branch_ref: &str) -> Result<git2::Commit<'_>, git2::Error> {
        let commit = self.repository.find_commit(oid)?;

        let repo_dir = self.dir.clone();
        let git_dir = format!("{}{}", &repo_dir, ".git");
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&git_dir)
            .arg("archive")
            .arg("--format=tar")
            .arg(oid.to_string())
            .output().expect("Should work");

        if !output.status.success() {
            return Err(git2::Error::from_str(&format!(
            "Failed to create archive: {}",
            String::from_utf8_lossy(&output.stderr)
            )));
        }

        if !std::path::Path::new(&repo_dir).exists() {
            std::fs::create_dir_all(&repo_dir).expect("Should work");
        } else {
            std::fs::remove_dir_all(&repo_dir).expect("Should work");
            std::fs::create_dir_all(&repo_dir).expect("Should work");
        }

        let cursor = Cursor::new(output.stdout);
        let mut archive = tar::Archive::new(cursor);
        archive.unpack(&repo_dir).map_err(|e| {
            git2::Error::from_str(&format!("Failed to unpack tar archive: {}", e))
        })?;

        Ok(commit)
    }
}