use std::{collections::HashSet, fs, io::Write, path::Path};

use clap::{Parser, Subcommand};
use dotenv::dotenv;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

mod gather;
mod repository;
mod code;
mod analyze;
mod pipeline;
mod consts;
mod math;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Collect {
        #[arg(
            long,
            help = "Minimum number of stars a repository must have to be included",
        )]
        max_stars: Option<u32>,

        #[arg(
            long,
            help = "Results CSV file path",
            default_value = "results/repositories.csv",
        )]
        output: String,
    },
    Analysis {
        #[arg(
            long,
            help = "Input CSV file path",
            default_value = "results/repositories.csv",
        )]
        input: String,

        #[arg(
            long,
            help = "Output CSV file path",
            default_value = "results/analysis",
        )]
        output: String,
    },
    Single {
        name: String,
        output: String,
    },
    Compare {
        from: String,
        to: String,
    },
    Symbols {
        #[arg(
            long,
            help = "Minimum number of stars",
            default_value = "500",
        )]
        min_stars: u32,
    }
}

#[tokio::main]
async fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let (ctrlc_tx, ctrlc_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
        let _ = ctrlc_tx.send(());
    });

    #[cfg(feature = "dhat-heap")]
    let profiler = Some(_profiler);

    tokio::spawn(async move {
        ctrlc_rx.await.ok();
        #[cfg(feature = "dhat-heap")]
        drop(profiler);
        std::process::exit(0);
    });

    let args = Args::parse();
    dotenv().ok();

    match &args.command {
        None => {
            println!("No command provided. Use --help for more information.");
        },
        Some(Commands::Collect {
            max_stars,
            output,
        }) => {
           let personal_token = std::env::var("GITHUB_PAT").expect("GITHUB_PAT not set in .env file");
            pipeline::run_collection_pipeline(&personal_token, *max_stars, output).await;
        }
        Some(Commands::Analysis {
            input,
            output,
        }) => {
            pipeline::run_analysis_pipeline(input, output);
        },
        Some(Commands::Compare {
            from,
            to,
        }) => {
            let from_symbols = gather::get_repo_symbols(from).expect("Failed to get symbols for 'from' repository");
            let to_symbols = gather::get_repo_symbols(to).expect("Failed to get symbols for 'to' repository");

            for from_lang in from_symbols.keys() {
                for to_lang in to_symbols.keys() {
                    let from_set = from_symbols.get(from_lang).unwrap();
                    let to_set = to_symbols.get(to_lang).unwrap();

                    let common: HashSet<_> = from_set.intersection(to_set).collect();
                    let common_count = common.len();
                    let from_pct = if !from_set.is_empty() {
                        common_count as f64 / from_set.len() as f64 * 100.0
                    } else {
                        0.0
                    };
                    let to_pct = if !to_set.is_empty() {
                        common_count as f64 / to_set.len() as f64 * 100.0
                    } else {
                        0.0
                    };

                    println!("Common symbols between {}'s {:?} and {}'s {:?}: {} (from {} or {:.2}% to {} or {:.2}%)", 
                        from,
                        from_lang,
                        to,
                        to_lang,
                        common_count,
                        from_set.len(),
                        from_pct,
                        to_set.len(),
                        to_pct
                    );
                }
            }
        }
        Some(Commands::Single {
            name,
            output,
        }) => {
            let parts: Vec<&str> = name.split('/').collect();
            if parts.len() != 2 {
                println!("[{}] Skipping invalid repository name", name);
                return;
            }

            let owner = parts[0];
            let repo = parts[1];
            let result_folder = format!("{}/{}_{}", output, owner, repo);

            if !Path::new(&result_folder).exists() {
                fs::create_dir_all(&result_folder).expect("Failed to create output directory");
            }

            let result_file = format!("{}/result.txt", result_folder);
            let log_file = format!("{}/log.txt", result_folder);

            if Path::new(&result_file).exists() {
                println!("[{}] Output already exists, skipping", name);
                return;
            }

            let mut log_writer = fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&log_file)
                .expect("Failed to create log file");

            let time = chrono::Local::now();
            log_writer
                .write_all(format!("Starting analysis for repository: {} at {}\n", name, time).as_bytes())
                .expect("Failed to write to log file");

            let temp_dir = format!("temp/{}_{}", owner, repo);

            let gather_result = gather::gather_repository_statistics(
                owner,
                repo,
                &temp_dir,
                100,
                &mut log_writer.try_clone().expect("Failed to clone log writer")
            );
            
            match gather_result {
                Ok(stats) => {
                    let status = analyze::check_migration_status(
                        stats, 
                        &mut log_writer.try_clone().expect("Failed to clone log writer")
                    );
                    let mut result_writer = fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(&result_file)
                        .expect("Failed to create result file");

                    result_writer
                        .write_all(format!("{:?},{:?}", status.0, status.1).as_bytes())
                        .expect("Failed to write to result file");
                    println!("[{}] Result={:?},{:?}", name, status.0, status.1);
                }
                Err(e) => {
                    log_writer
                        .write_all(format!("Error during gathering: {:?}\n", e).as_bytes())
                        .expect("Failed to write to log file");
                    println!("[{}] Error during gathering: {:?}", name, e);
                }
            }
            pipeline::clean_temp_dir(&temp_dir);
        }
        Some(Commands::Symbols {
            min_stars
        }) => {
            pipeline::run_symbols_pipeline(&min_stars).await;
        }
    }
}