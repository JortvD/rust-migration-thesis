use common::input::{get_input, InputData};
use dotenv::dotenv;
use reqwest::{Client, StatusCode, header};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const RESET_BUFFER: u64 = 1;

async fn wait_for_rate_limit_reset(client: &Client) -> AppResult<()> {
    let response = client
        .get("https://api.github.com/rate_limit")
        .send()
        .await?;

    if response.status().is_success() {
        let rate_limit: Value = response.json().await?;
        
        let reset_time = rate_limit
            .pointer("/rate/reset")
            .and_then(|r| r.as_u64())
            .unwrap_or_else(|| {
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60
            });

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let wait_time = reset_time.saturating_sub(current_time) + RESET_BUFFER;
        
        eprintln!("Rate limit exceeded. Waiting for {} seconds...", wait_time);
        sleep(Duration::from_secs(wait_time)).await;
        Ok(())
    } else {
        Err(format!("Failed to fetch rate limit: {}", response.status()).into())
    }
}

async fn fetch_with_retry(client: &Client, url: &str) -> AppResult<Value> {
    loop {
        let response = client.get(url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let json: Value = response.json().await?;
                return Ok(json);
            }
            StatusCode::FORBIDDEN => {
                wait_for_rate_limit_reset(client).await?;
            }
            status => {
                return Err(format!("Request to {} failed with status: {}", url, status).into());
            }
        }
    }
}

async fn fetch_issue_timeline(client: &Client, url: &str, file_path: &Path) -> AppResult<()> {
    println!("Fetching timeline for issue at {}", url);
    
    let timeline = fetch_with_retry(client, url).await?;
    
    if let Some(timeline_arr) = timeline.as_array() {
        println!("Fetched timeline with {} events", timeline_arr.len());
        let json_string = serde_json::to_string_pretty(&timeline)?;
        fs::write(file_path, json_string).await?;
    }
    
    Ok(())
}

async fn fetch_issues(project: &InputData, results_folder: &Path, client: &Client) -> AppResult<()> {
    let mut page = 1;
    let mut since = "2000-01-01T00:00:00Z".to_string();
    let mut latest_since = since.clone();

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues?sort=updated&direction=asc&state=all&per_page=100&page={}&since={}",
            project.author, project.name, page, since
        );

        println!("Fetching issues for {}/{} on page {} from {}", project.author, project.name, page, since);

        let json_response = fetch_with_retry(client, &url).await?;
        let page_issues = json_response.as_array().ok_or("API did not return a JSON array")?;

        if page_issues.is_empty() {
            break;
        }

        let num_prs = page_issues.iter().filter(|issue| issue.get("pull_request").is_some()).count();

        println!("Fetched {} issues (of which {} are pull requests) on page {} from {}", page_issues.len(), num_prs, page, since);

        for issue in page_issues {
            // Skip pull requests
            if issue.get("pull_request").is_some() {
                continue; 
            }

            let issue_number = issue.get("number").and_then(|n| n.as_i64()).unwrap_or(0);
            let updated_at = issue.get("updated_at").and_then(|s| s.as_str()).unwrap_or(&latest_since);
            latest_since = updated_at.to_string();
            
            let issue_file_path = results_folder.join(format!("issue_{}.json", issue_number));
            if issue_file_path.exists() {
                println!("Issue {} already exists, skipping...", issue_number);
                continue;
            }
            let issue_json = serde_json::to_string_pretty(&issue)?;
            fs::write(&issue_file_path, issue_json).await?;

            if let Some(issue_url) = issue.get("timeline_url").and_then(|u| u.as_str()) {
                let timeline_file_path = results_folder.join(format!("issue_{}_timeline.json", issue_number));
                
                if let Err(e) = fetch_issue_timeline(client, issue_url, &timeline_file_path).await {
                    eprintln!("Failed to fetch timeline for issue {}: {}", issue_number, e);
                }
                
                sleep(Duration::from_millis(100)).await;
            }
        }

        // If we received fewer than 100 items, we've hit the last page
        if page_issues.len() < 100 {
            break;
        }

        if page == 99 {
            since = latest_since.clone();
            page = 1;
            println!("Reached page 100, updating 'since' to {} and resetting page to 1", since);
        } else {
            page += 1;
        }

        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

fn build_client() -> AppResult<Client> {
    let token = std::env::var("GITHUB_PAT").map_err(|_| "GITHUB_PAT environment variable not set")?;
    
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", token).parse()?,
    );
    headers.insert(
        header::USER_AGENT,
        "Rust Issue Fetcher".parse()?,
    );

    let client = Client::builder()
        .default_headers(headers)
        .build()?;
        
    Ok(client)
}

async fn fetch_releases(project: &InputData, results_folder: &Path, client: &Client) -> AppResult<()> {
    let mut page = 1;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=100&page={}",
            project.author, project.name, page
        );

        println!("Fetching releases for {}/{} on page {}", project.author, project.name, page);

        let json_response = fetch_with_retry(client, &url).await?;
        let page_releases = json_response.as_array().ok_or("API did not return a JSON array")?;

        if page_releases.is_empty() {
            break;
        }

        println!("Fetched {} releases on page {}", page_releases.len(), page);

        for release in page_releases {
            // We use the release 'id' to create a unique filename. 
            // You could alternatively use 'tag_name' if you prefer text-based filenames.
            let release_id = release.get("id").and_then(|n| n.as_i64()).unwrap_or(0);
            
            let release_file_path = results_folder.join(format!("release_{}.json", release_id));
            let release_json = serde_json::to_string_pretty(&release)?;
            fs::write(&release_file_path, release_json).await?;
        }

        if page_releases.len() < 100 {
            break;
        }

        page += 1;
        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

async fn fetch_security_advisories(project: &InputData, results_folder: &Path, client: &Client) -> AppResult<()> {
    let mut page = 1;
    let mut id = 1;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/security-advisories?per_page=100&page={}",
            project.author, project.name, page
        );

        println!("Fetching security advisories for {}/{} on page {}", project.author, project.name, page);

        // Security advisories can sometimes return 403/404 if not enabled or no permissions
        let response = client.get(&url).send().await?;
        
        if response.status() == StatusCode::NOT_FOUND || response.status() == StatusCode::FORBIDDEN {
            eprintln!("Security advisories are disabled or inaccessible for {}/{}", project.author, project.name);
            break;
        }

        let json_response: Value = response.json().await?;
        let page_advisories = json_response.as_array().ok_or("API did not return a JSON array for advisories")?;

        if page_advisories.is_empty() {
            break;
        }

        println!("Fetched {} security advisories on page {}", page_advisories.len(), page);

        for advisory in page_advisories {
            let advisory_file_path = results_folder.join(format!("advisory_{}.json", id));
            let advisory_json = serde_json::to_string_pretty(&advisory)?;
            fs::write(&advisory_file_path, advisory_json).await?;
            id += 1;
        }

        if page_advisories.len() < 100 {
            break;
        }

        page += 1;
        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

enum OwnerType {
    User,
    Organization,
}
struct ProjectCharacteristics {
    project: String,
    original_project: Option<String>,
    same_owner: bool,
    owner_type: OwnerType,
    archived: bool,
    original_archived: Option<bool>,
    stars: u64,
    original_stars: Option<u64>,
    forks: u64,
    original_forks: Option<u64>,
    size: u64,
    original_size: Option<u64>,
    rust_percentage: f64,
}

impl ProjectCharacteristics {
    fn to_header() -> String {
        "project,original_project,same_owner,owner_type,archived,original_archived,stars,original_stars,forks,original_forks,size,original_size,rust_percentage\n".to_string()
    }

    fn to_csv(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            self.project,
            self.original_project.as_deref().unwrap_or(""),
            self.same_owner,
            match self.owner_type {
                OwnerType::User => "User",
                OwnerType::Organization => "Organization",
            },
            self.archived,
            self.original_archived.map(|a| a.to_string()).unwrap_or_else(|| "".to_string()),
            self.stars,
            self.original_stars.map(|s| s.to_string()).unwrap_or_else(|| "".to_string()),
            self.forks,
            self.original_forks.map(|f| f.to_string()).unwrap_or_else(|| "".to_string()),
            self.size,
            self.original_size.map(|s| s.to_string()).unwrap_or_else(|| "".to_string()),
            self.rust_percentage,
        )
    }
}

async fn fetch_characteristics(project: &InputData, client: &Client, writer: &mut fs::File) -> AppResult<()> {
    println!("Fetching characteristics for {}/{} and {}/{}", project.author, project.name, project.orig_author.as_deref().unwrap_or(""), project.orig_name.as_deref().unwrap_or(""));
    let project_response = fetch_with_retry(client, &format!(
        "https://api.github.com/repos/{}/{}",
        project.author, project.name
    )).await?;

    let languages_response = fetch_with_retry(client, &format!(
        "https://api.github.com/repos/{}/{}/languages",
        project.author, project.name
    )).await?;
    let size = languages_response.as_object().map(|langs| langs.values().filter_map(|v| v.as_u64()).sum()).unwrap_or(0);
    let rust_percentage = languages_response.get("Rust").and_then(|v| v.as_u64()).map(|rust_size| rust_size as f64 / size as f64 * 100.0).unwrap_or(0.0);
    
    let characteristics = if let Some(original_author) = &project.orig_author && let Some(original_name) = &project.orig_name {
        let original_project_response = fetch_with_retry(client, &format!(
            "https://api.github.com/repos/{}/{}",
            original_author, original_name
        )).await?;
        let original_languages_response = fetch_with_retry(client, &format!(
            "https://api.github.com/repos/{}/{}/languages",
            original_author, original_name
        )).await?;

        ProjectCharacteristics {
            project: format!("{}/{}", project.author, project.name),
            original_project: Some(format!("{}/{}", original_author, original_name)),
            same_owner: project.author == *original_author,
            owner_type: if project_response.get("owner").and_then(|o| o.get("type")).and_then(|t| t.as_str()) == Some("Organization") {
                OwnerType::Organization
            } else {
                OwnerType::User
            },
            archived: project_response.get("archived").and_then(|a| a.as_bool()).unwrap_or(false),
            original_archived: original_project_response.get("archived").and_then(|a| a.as_bool()).map(Some).unwrap_or(None),
            stars: project_response.get("stargazers_count").and_then(|s| s.as_u64()).unwrap_or(0),
            original_stars: original_project_response.get("stargazers_count").and_then(|s| s.as_u64()).map(Some).unwrap_or(None),
            forks: project_response.get("forks_count").and_then(|f| f.as_u64()).unwrap_or(0),
            original_forks: original_project_response.get("forks_count").and_then(|f| f.as_u64()).map(Some).unwrap_or(None),
            size: size,
            original_size: original_languages_response.as_object().map(|langs| langs.values().filter_map(|v| v.as_u64()).sum()).map(Some).unwrap_or(None),
            rust_percentage: rust_percentage,
        }
    } else {
        ProjectCharacteristics {
            project: format!("{}/{}", project.author, project.name),
            original_project: None,
            same_owner: true,
            owner_type: if project_response.get("owner").and_then(|o| o.get("type")).and_then(|t| t.as_str()) == Some("Organization") {
                OwnerType::Organization
            } else {
                OwnerType::User
            },
            archived: project_response.get("archived").and_then(|a| a.as_bool()).unwrap_or(false),
            original_archived: None,
            stars: project_response.get("stargazers_count").and_then(|s| s.as_u64()).unwrap_or(0),
            original_stars: None,
            forks: project_response.get("forks_count").and_then(|f| f.as_u64()).unwrap_or(0),
            original_forks: None,
            size: size,
            original_size: None,
            rust_percentage: rust_percentage,
        }
    };

    writer.write_all(characteristics.to_csv().as_bytes()).await?;

    Ok(())
}

async fn fetch_all(author: String, name: String, results_folder: &Path, client: &Client) -> AppResult<()> {
    let project = InputData { author, name, orig_author: None, orig_name: None };
    
    if !results_folder.exists() {
        fs::create_dir_all(results_folder).await?;
    }

    fetch_issues(&project, results_folder, client).await?;
    fetch_releases(&project, results_folder, client).await?;
    fetch_security_advisories(&project, results_folder, client).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenv().ok();

    let client = build_client()?;
    let lines = get_input("input.txt").expect("Failed to read input.txt");

    let result_root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("missing result path argument")?,
    );

    let characteristics_file = result_root.join("characteristics.csv");
    let mut writer = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&characteristics_file)
        .await?;
    writer.write_all(ProjectCharacteristics::to_header().as_bytes()).await?;

    for line in lines {
        let results_folder = PathBuf::from(format!("{}/{}_{}", result_root.display(), line.author, line.name));

        fetch_characteristics(&line, &client, &mut writer).await?;
        
        // if results_folder.exists() {
        //     println!("Results folder for {} already exists", &line.name);
        // } else {
        //     fs::create_dir_all(&results_folder).await?;
        // }

        // if let Err(e) = fetch_all(line.author.clone(), line.name.clone(), &results_folder, &client).await {
        //     eprintln!("Failed to fetch data for {}/{}: {}", line.author, line.name, e);
        // }

        // if let Some(orig_author) = &line.orig_author && let Some(orig_name) = &line.orig_name {
        //     let orig_results_folder = PathBuf::from(format!("{}/{}_{}", result_root.display(), orig_author, orig_name));
            
        //     if orig_results_folder.exists() {
        //         println!("Results folder for original project {} already exists", &orig_name);
        //     } else {
        //         fs::create_dir_all(&orig_results_folder).await?;
        //     }

        //     if let Err(e) = fetch_all(orig_author.clone(), orig_name.clone(), &orig_results_folder, &client).await {
        //         eprintln!("Failed to fetch data for original project {}/{}: {}", orig_author, orig_name, e);
        //     }
        // }
    }

    Ok(())
}