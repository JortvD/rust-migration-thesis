use clap::{Parser, Subcommand};
use dotenv::dotenv;

mod gather;
mod repository;
mod plots;
mod code;
mod analyze;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Tokei {
        owner: String,
        repo: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,

        #[arg(short, long, help = "Number of commits to analyze", default_value_t = 100)]
        num_commits: usize,

        #[arg(
            long,
            help = "Generate language division plot over time"
        )]
        plot: bool,

        #[arg(
            long,
            help = "Test for migration to Rust"
        )]
        test: bool,
    },
    Cargo {
        owner: String,
        repo: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,

        #[arg(short, long, help = "Number of commits to analyze", default_value_t = 100)]
        num_commits: usize,

        #[arg(
            long,
            help = "Generate Cargo.toml amount plot over time"
        )]
        plot: bool,

        #[arg(
            long,
            help = "Test for migration to Rust"
        )]
        test: bool,
    },
    Matches {
        owner: String,
        repo: String,

        commit1_hash: String,
        commit2_hash: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,

        #[arg(
            long,
            help = "Generate histogram of matching symbols between the two commits"
        )]
        plot: bool,

        #[arg(
            long,
            help = "Test for migration to Rust"
        )]
        test: bool,
    },
    Matches2 {
        owner: String,
        repo: String,

        commit_hash: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,

        #[arg(
            long,
            help = "Generate series of matching symbols analysis"
        )]
        plot: bool,
    },
    Overlap {
        owner1: String,
        repo1: String,
        owner2: String,
        repo2: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,
    },
    Command {
        owner: String,
        repo: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,

        #[arg(short, long, help = "Number of commits to analyze", default_value_t = 100)]
        num_commits: usize,

        #[arg(
            long,
            help = "Generate command usage plot over time"
        )]
        plot: bool,
    },
    Text {
        owner: String,
        repo: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,

        #[arg(short, long, help = "Number of commits to analyze", default_value_t = 100)]
        num_commits: usize,

        #[arg(
            long,
            help = "Generate text analysis plots over time"
        )]
        plot: bool,
    },
    Test {
        owner: String,
        repo: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,
    },
    FindRepositories {
        #[arg(
            long,
            help = "Minimum number of stars a repository must have to be included",
        )]
        max_stars: Option<u32>,
    },
    Count {},
    Status {
        owner: String,
        repo: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    dotenv().ok();

    match &args.command {
        Some(Commands::Tokei {
            owner,
            repo,
            temp_dir,
            num_commits,
            plot,
            test,
        }) => {
            match gather::tokei(owner, repo, temp_dir, *num_commits) {
                Ok(result) => {
                    if *plot {
                        if let Err(e) = plots::plot_language_division(&result, owner, repo) {
                            eprintln!("Failed to generate division plot: {:?}", e);
                        }
                    }
                    if *test {
                        let is_migration = analyze::tokei_migration_test(result);
                        if is_migration {
                            println!("The repository shows signs of migration to Rust.");
                        } else {
                            println!("No significant migration to Rust detected.");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Cargo {
            owner,
            repo,
            temp_dir,
            num_commits,
            plot,
            test,
        }) => {
            match gather::cargo(owner, repo, temp_dir, *num_commits) {
                Ok(result) => {
                    if *plot {
                        if let Err(e) = plots::plot_toml_amount(&result, owner, repo) {
                            eprintln!("Failed to generate Cargo.toml amount plot: {:?}", e);
                        }
                    }

                    if *test {
                        let is_migration = analyze::cargo_migration_test(&result);
                        if is_migration {
                            println!("The repository shows signs of migration to Rust.");
                        } else {
                            println!("No significant migration to Rust detected.");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Cargo analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Matches {
            owner, 
            repo, 
            commit1_hash,
            commit2_hash, 
            temp_dir, 
            plot,
            test,
        }) => {
            match gather::matches(owner, repo, temp_dir, commit1_hash, commit2_hash) {
                Ok(result) => {
                    if *plot {
                        if let Err(e) = plots::plot_matching_symbols_histogram(&result, owner, repo) {
                            eprintln!("Failed to generate matching symbols histogram: {:?}", e);
                        }
                    }
                    if *test {
                        let is_migration = analyze::matches_migration_test(result);
                        if is_migration {
                            println!("The repository shows signs of migration to Rust based on matching symbols.");
                        } else {
                            println!("No significant migration to Rust detected based on matching symbols.");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Matching substrings analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Matches2 {
            owner, 
            repo, 
            commit_hash, 
            temp_dir, 
            plot,
        }) => {
            match gather::matches2(owner, repo, temp_dir, commit_hash) {
                Ok(result) => {
                    println!("Matches2 analysis completed for {}/{} at commit {}.", owner, repo, commit_hash);
                    if *plot {
                        if let Err(e) = plots::plot_matches2(&result, owner, repo) {
                            eprintln!("Failed to generate Matches2 analysis plots: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Matches2 analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Overlap {
            owner1,
            repo1,
            owner2,
            repo2,
            temp_dir,
        }) => {
            match gather::overlap(owner1, repo1, owner2, repo2, temp_dir) {
                Ok(_) => {
                    println!("Overlap analysis completed for {}/{} and {}/{}.", owner1, repo1, owner2, repo2);
                }
                Err(e) => {
                    eprintln!("Overlap analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Command {
            owner,
            repo,
            temp_dir,
            num_commits,
            plot,
        }) => {
            match gather::commands(owner, repo, temp_dir, *num_commits) {
                Ok(result) => {
                    if *plot {
                        if let Err(e) = plots::plot_command_usage(&result, owner, repo) {
                            eprintln!("Failed to generate command usage plot: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Command analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Text {
            owner,
            repo,
            temp_dir,
            num_commits,
            plot,
        }) => {
            match gather::text(owner, repo, temp_dir, *num_commits) {
                Ok(result) => {
                    println!("Text analysis completed for {}/{}.", owner, repo);
                    if *plot {
                        if let Err(e) = plots::plot_text_analysis(&result, owner, repo) {
                            eprintln!("Failed to generate text analysis plots: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Text analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Test {
            owner,
            repo,
            temp_dir,
        }) => {
            println!("Running full migration test for {}/{}...", owner, repo);
            let mut count = 0;
            match gather::tokei(owner, repo, temp_dir, 100) {
                Ok(tokei_result) => {
                    let tokei_migration = analyze::tokei_migration_test(tokei_result);
                    if tokei_migration {
                        println!("Tokei analysis indicates migration to Rust.");
                        count += 1;
                    } else {
                        println!("Tokei analysis does not indicate migration to Rust.");
                    }
                }
                Err(e) => {
                    eprintln!("Tokei analysis failed: {:?}", e);
                }
            }
            let mut commit_before: Option<String> = None;
            let mut commit_after: Option<String> = None;

            match gather::cargo(owner, repo, temp_dir, 100) {
                Ok(cargo_result) => {
                    let cargo_migration = analyze::cargo_migration_test(&cargo_result);
                    if cargo_migration {
                        println!("Cargo analysis indicates migration to Rust.");
                        count += 1;
                    } else {
                        println!("Cargo analysis does not indicate migration to Rust.");
                    }

                    let (before, after) = analyze::cargo_find_before_after(&cargo_result);
                    println!("Identified commits for Matches analysis:");
                    println!("Before migration commit: {}", before);
                    println!("After migration commit: {}", after);
                    commit_before = Some(before);
                    commit_after = Some(after);
                }
                Err(e) => {
                    eprintln!("Cargo analysis failed: {:?}", e);
                }
            }

            if commit_before.is_none() || commit_after.is_none() {
                return;
            }

            match gather::matches(
                owner,
                repo,
                temp_dir,
                &commit_before.unwrap(),
                &commit_after.unwrap(),
            ) {
                Ok(matches_result) => {
                    let matches_migration = analyze::matches_migration_test(matches_result);
                    if matches_migration {
                        println!("Matches analysis indicates migration to Rust.");
                        count += 1;
                    } else {
                        println!("Matches analysis does not indicate migration to Rust.");
                    }
                }
                Err(e) => {
                    eprintln!("Matches analysis failed: {:?}", e);
                }
            }

            println!("Migration test completed. Indicators of migration to Rust: {}/3", count);
        }
        None => {
            println!("No command provided. Use --help for more information.");
        },
        Some(Commands::FindRepositories {
            max_stars,
        }) => {
            let personal_token = std::env::var("GITHUB_PAT").expect("GITHUB_PAT not set in .env file");
            let instance = octocrab::Octocrab::builder()
                .personal_token(personal_token)
                .build().unwrap();

            let mut stars = max_stars.unwrap_or(10_000_000);
            let mut page_num = 1u32;
            let mut i = 0;
            let mut previous_repositories: Vec<String> = Vec::new();
            let mut current_repositories: Vec<String> = Vec::new();

            loop {
                println!("Fetching repositories with up to {} stars...", stars);
                let page: octocrab::Page<octocrab::models::Repository> = instance
                    .search()
                    .repositories(&format!("stars:<={}", stars))
                    .sort("stars")
                    .order("desc")
                    .per_page(100)
                    .page(page_num)
                    .send()
                    .await.expect("Help");
                let mut changed = false;
                let old_stars = stars;

                println!("Processing {} repositories from page {}...", page.items.len(), page_num);

                let mut wtr = csv::WriterBuilder::new()
                .has_headers(false) // Avoid writing headers again
                .from_writer(std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open("results/repositories3.csv")
                    .expect("Failed to open CSV file"));

                for repo in page.items {
                    let full_name = repo.full_name.unwrap();
                    current_repositories.push(full_name.clone());

                    if repo.stargazers_count.is_none() {
                        println!("E({}-{}) {} -> no stars info.", i, 0, full_name);
                        continue;
                    }

                    if previous_repositories.contains(&full_name) {
                        println!(",({}-{}) {} -> already processed.", i, repo.stargazers_count.unwrap_or(0), full_name);
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }

                    stars = repo.stargazers_count.unwrap_or(0);
                    changed |= stars < old_stars;

                    let languages = instance
                        .repos(&repo.owner.unwrap().login, &repo.name)
                        .list_languages()
                        .await.expect("Failed to fetch languages");

                    let max_lang = languages.iter().max_by_key(|entry| entry.1).map(|(lang, _)| lang.clone()).unwrap_or("Unknown".to_string());
                    i += 1;

                    if !languages.contains_key("Rust") {
                        println!(".({}-{}) {} -> no Rust (max: {}).", i, repo.stargazers_count.unwrap_or(0), full_name, max_lang);
                        continue;
                    }

                    let code_sum = languages.values().sum::<i64>() as f64;
                    let rust_percentage = (languages.get("Rust").unwrap_or(&0).clone() as f64 / code_sum) * 100.0;

                    println!("+({}-{}) {} -> {:.2}% Rust (max: {})", i, repo.stargazers_count.unwrap_or(0), full_name, rust_percentage, max_lang);
                    wtr.write_record(&[
                        max_lang, 
                        full_name, 
                        repo.stargazers_count.unwrap_or(0).to_string(), 
                        format!("{:.2}", rust_percentage),
                        format!("{:?}", languages)
                    ])
                        .expect("Failed to write record");

                    wtr.flush().expect("Failed to flush CSV writer");
                }

                println!("Decrease stars threshold from {} to {} for next page (= {}).", old_stars, stars, old_stars - stars);

                if !changed {
                    page_num += 1;
                } else {
                    page_num = 1;
                }

                previous_repositories = current_repositories.clone();
                current_repositories.clear();
            }
        }
        Some(Commands::Count {}) => {
            let mut rdr = csv::Reader::from_path("results/repositories3.csv").expect("Failed to open CSV file");
            let mut count = 0;
            let mut seen_repositories = std::collections::HashSet::new();
            let mut common_languages = std::collections::HashMap::new();

            for result in rdr.records() {
                let record = result.expect("Failed to read record");
                let rust_percentage: f64 = record[3].parse().unwrap_or(0.0);

                let is_new = seen_repositories.insert(record[1].to_string());

                if rust_percentage >= 1.0 && is_new {
                    let max_lang = record[0].to_string();
                    *common_languages.entry(max_lang.clone()).or_insert(0) += 1;
                    count += 1;
                }
            }

            println!("Repos with at least 1% Rust: {}", count);

            println!("Most common languages:");
            let mut common_languages_vec: Vec<_> = common_languages.iter().collect();
            common_languages_vec.sort_by(|a, b| b.1.cmp(&a.1));
            for (lang, lang_count) in common_languages_vec.into_iter().take(10) {
                println!("- {}: {}", lang, lang_count);
            }
        }
        Some(Commands::Status {
            owner,
            repo,
            temp_dir,
        }) => {
            match gather::determine_status(owner, repo, temp_dir, 100) {
                Ok(status) => {
                    println!("Migration status for {}/{}:", owner, repo);
                    println!("Minimum Rust percentage: {:.2}%", status.min_rust * 100.0);
                    println!("Maximum Rust percentage: {:.2}%", status.max_rust * 100.0);
                    println!("Peak code moved to Rust: {:.2}%", status.peak_moved * 100.0);
                }
                Err(e) => {
                    eprintln!("Failed to get migration status: {:?}", e);
                }
            }
        }
    }
}
