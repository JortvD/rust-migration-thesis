use common::input::{get_input, InputData};
use dotenv::dotenv;
use reqwest::{Client, StatusCode, header};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
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

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues?state=all&per_page=100&page={}",
            project.author, project.name, page
        );

        println!("Fetching issues for {}/{} on page {}", project.author, project.name, page);

        let json_response = fetch_with_retry(client, &url).await?;
        let page_issues = json_response.as_array().ok_or("API did not return a JSON array")?;

        if page_issues.is_empty() {
            break;
        }

        println!("Fetched {} issues on page {}", page_issues.len(), page);

        for issue in page_issues {
            // Skip pull requests
            if issue.get("pull_request").is_some() {
                continue; 
            }

            let issue_number = issue.get("number").and_then(|n| n.as_i64()).unwrap_or(0);
            
            let issue_file_path = results_folder.join(format!("issue_{}.json", issue_number));
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

        page += 1;
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

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenv().ok();

    let client = build_client()?;
    let lines = get_input("input.txt").expect("Failed to read input.txt");

    for line in lines {
        let results_folder = PathBuf::from(format!("results/{}_{}", line.author, line.name));
        
        if results_folder.exists() {
            eprintln!("Results folder for {} already exists, skipping...", &line.name);
            continue;
        } else {
            fs::create_dir_all(&results_folder).await?;
        }

        // if let Err(e) = fetch_issues(&line, &results_folder, &client).await {
        //     eprintln!("Error fetching issues for {}: {}", &line.name, e);
        // }

        // if let Err(e) = fetch_releases(&line, &results_folder, &client).await {
        //     eprintln!("Error fetching releases for {}: {}", &line.name, e);
        // }

        if let Err(e) = fetch_security_advisories(&line, &results_folder, &client).await {
            eprintln!("Error fetching security advisories for {}: {}", &line.name, e);
        }
    }

    Ok(())
}