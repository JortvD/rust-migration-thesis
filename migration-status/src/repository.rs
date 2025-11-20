use git2::Repository;
use std::time::Instant;

pub struct RepositoryInfo {
    pub repository: Repository,
}

impl RepositoryInfo {
    pub fn clone_or_open(owner: &str, repo: &str, dir: &str) -> Result<Self, git2::Error> {
        let start_time = Instant::now();
        
        let repo_url = format!("https://github.com/{}/{}.git", owner, repo);
        let repository = if !std::path::Path::new(&dir).exists() {
            match Repository::clone(&repo_url, &dir) {
                Ok(repo) => repo,
                Err(e) => return Err(e),
            }
        } else {
            match Repository::open(&dir) {
                Ok(repo) => repo,
                Err(e) => return Err(e),
            }
        };
        println!("Cloned/opened repository in {} ms", start_time.elapsed().as_millis());

        Ok(RepositoryInfo {
            repository,
        })
    }

    pub fn get_main_branch(&self) -> Option<String> {
        if self.repository.find_branch("main", git2::BranchType::Local).is_ok() {
            Some("refs/heads/main".to_string())
        } else if self.repository.find_branch("master", git2::BranchType::Local).is_ok() {
            Some("refs/heads/master".to_string())
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
        let tree = commit.tree()?;
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.force();
        self.repository.checkout_tree(&tree.as_object(), Some(&mut checkout_builder))?;
        self.repository.set_head(branch_ref)?;
        Ok(commit)
    }
}