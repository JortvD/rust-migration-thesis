use git2::{FetchOptions, RemoteCallbacks};
use git2::{Repository, build::RepoBuilder};
use indicatif::ProgressBar;
use std::process::{Command, Stdio};
use std::time::Instant;

pub struct BareRepositoryInfo {
    pub dir: String,
    pub repository: Repository,
}

impl BareRepositoryInfo {
    pub fn clone_or_open(bar: &ProgressBar, owner: &str, repo: &str, dir: &str) -> Result<Self, git2::Error> {   
        let name = format!("{}/{}", owner, repo);    
        let repo_url = format!("https://github.com/{}/{}.git", owner, repo);
        let git_dir = dir.to_string() + ".git";
        let git_path = std::path::Path::new(&git_dir);
        let start_time = Instant::now();
        let repository = if !git_path.exists() {
            let mut callbacks = RemoteCallbacks::new();
            
            let mut last_received_pct = 0;
            let mut last_indexed_pct = 0;
            callbacks.transfer_progress(|stats| {
                let received_pct = if stats.total_objects() > 0 {
                    (stats.received_objects() * 100 / stats.total_objects()) as u32
                } else {
                    100
                };
                if received_pct > last_received_pct {
                    last_received_pct = received_pct;
                    bar.set_message(format!(
                        "{}: [download] received {} MB, {}/{} objects",
                        name,
                        stats.received_bytes() / (1024 * 1024),
                        stats.received_objects(),
                        stats.total_objects()
                    ));
                }
                let indexed_pct = if stats.total_objects() > 0 {
                    (stats.indexed_objects() * 100 / stats.total_objects()) as u32
                } else {
                    100
                };
                if indexed_pct > last_indexed_pct {
                    last_indexed_pct = indexed_pct;
                    bar.set_message(format!(
                        "{}: [indexing] {}/{} objects",
                        name,
                        stats.indexed_objects(),
                        stats.total_objects()
                    ));
                }
                true
            });

            callbacks.sideband_progress(|data| {
                let progress = String::from_utf8_lossy(data);
                bar.set_message(format!("{}: [unpacking] {}", name, progress.trim()));
                true
            });

            callbacks.pack_progress(|stage, current, total| {
                bar.set_message(format!(
                    "{}: [pack] stage: {:?}, {}/{} objects",
                    name,
                    stage,
                    current,
                    total
                ));
            });

            let mut fetch_opts = FetchOptions::new();
            fetch_opts.remote_callbacks(callbacks);

            match RepoBuilder::new()
                .bare(true)
                .fetch_options(fetch_opts)
                .clone(&repo_url, &git_path) {
                Ok(repo) => repo,
                Err(e) => return Err(e),
            }
        } else {
            match Repository::open_bare(&git_dir) {
                Ok(repo) => repo,
                Err(e) => return Err(e),
            }
        };

        bar.set_message(format!(
            "[{}] Repository at {} ready (took {} ms)",
            name,
            dir,
            start_time.elapsed().as_millis()
        ));

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

    pub fn get_latest_commit(&self, branch_ref: &str) -> Result<git2::Oid, git2::Error> {
        let reference = self.repository.find_reference(&branch_ref)?;
        let oid = reference
            .target()
            .ok_or_else(|| git2::Error::from_str("Invalid reference target"))?;
        Ok(oid)
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

        let mut child = Command::new("git")
            .arg("-C")
            .arg(&git_dir)
            .arg("archive")
            .arg("--format=tar")
            .arg(oid.to_string())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| git2::Error::from_str(&format!("Failed to spawn git archive: {}", e)))?;

        if let Some(stdout) = child.stdout.take() {
            if std::path::Path::new(&repo_dir).exists() {
                std::fs::remove_dir_all(&repo_dir).map_err(|e| {
                    git2::Error::from_str(&format!("Failed to remove working dir {}: {}", repo_dir, e))
                })?;
            }
            std::fs::create_dir_all(&repo_dir).map_err(|e| {
                git2::Error::from_str(&format!("Failed to create working dir {}: {}", repo_dir, e))
            })?;

            let mut archive = tar::Archive::new(stdout);
            archive.unpack(&repo_dir).map_err(|e| {
                git2::Error::from_str(&format!("Failed to unpack tar archive: {}", e))
            })?;
        } else {
            return Err(git2::Error::from_str("git child had no stdout"));
        }

        let status = child.wait().map_err(|e| {
            git2::Error::from_str(&format!("Failed waiting for git archive to finish: {}", e))
        })?;
        if !status.success() {
            return Err(git2::Error::from_str(&format!("git archive failed with status: {:?}", status)));
        }

        Ok(commit)
    }
}