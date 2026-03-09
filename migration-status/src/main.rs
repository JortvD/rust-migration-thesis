use std::{collections::HashSet, fs, io::{BufRead, Write}, path::Path};

use clap::{Parser, Subcommand};
use dotenv::dotenv;
use probminhash::superminhasher2::get_jaccard_index_estimate;
use rayon::ThreadPoolBuilder;

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
mod hash;

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
    Identifiers {
        name: String,
        from: usize,
        to: usize,
    },
    Sim {
        from: String,
        to: String,
    },
    Compare {
        from: String,
        to: String,
    },
    Symbols {
        #[arg(
            long,
            help = "Input CSV file path",
            default_value = "results/all_repositories.csv",
        )]
        input: String,

        #[arg(
            long,
            help = "Output folder path",
            default_value = "results/symbols",
        )]
        output: String,
    },
    SymbolsSingle {
        name: String,
        main_branch: String,

        #[arg(
            long,
            help = "Output folder path",
            default_value = "results/symbols",
        )]
        output: String,
    },
    SymbolsCollect {
        #[arg(
            long,
            help = "Minimum number of stars",
            default_value = "250",
        )]
        min_stars: u32,
    },
    SymbolsHash {
        #[arg(
            long,
            help = "Input folder path",
            default_value = "results/symbols",
        )]
        input: String,
        
        #[arg(
            long,
            help = "Output binary path",
            default_value = "results/symbols_hash.bin",
        )]
        output: String,
    },
    SymbolsCompare {
        from: String,

        #[arg(
            long,
            help = "Input binary file path",
            default_value = "results/symbols_hash.bin",
        )]
        input: String,

        #[arg(
            long,
            help = "Results folder path",
            default_value = "results/symbols",
        )]
        results: String,

        #[arg(
            long,
            help = "Language filter",
            default_value = "Rust",
        )]
        language: String,
    },
    SymbolsCompareAll {
        #[arg(
            long,
            help = "Input binary file path",
            default_value = "results/symbols_hash_5.bin",
        )]
        input: String,

        #[arg(
            long,
            help = "Repositories CSV file path",
            default_value = "results/repositories_correct.csv",
        )]
        repositories: String,

        #[arg(
            long,
            help = "Output data file",
            default_value = "results/symbols_compare.csv",
        )]
        output: String,
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

    ThreadPoolBuilder::new()
        .stack_size(16 * 1024 * 1024)
        .build_global().unwrap();

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
            let pb = indicatif::ProgressBar::new(0);
            pb.set_style(indicatif::ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] {msg}").unwrap()
                .progress_chars("#>-"));
            pipeline::run_analysis_for_repo(name, output, &pb);
        }
        Some(Commands::Identifiers {
            name,
            from,
            to,
        }) => {
            pipeline::run_identifiers_pipeline(name, *from, *to);
        }
        Some(Commands::Sim {
            from,
            to,
        }) => {
            pipeline::find_similar_symbols(from, to);
        }
        Some(Commands::Symbols { input, output }) => {
            pipeline::run_symbols_pipeline(input, output);
        }
        Some(Commands::SymbolsCollect {
            min_stars
        }) => {
            pipeline::run_symbols_collect_pipeline(&min_stars).await;
        }
        Some(Commands::SymbolsSingle { name, main_branch, output }) => {
            let pb = indicatif::ProgressBar::new(100);
            pipeline::run_symbols_for_repo(&pipeline::Repo {
                name: name.clone(),
                main_branch: main_branch.clone(),
                stars: 0,
                forks: 0,
                is_fork: false,
                size: 0,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                language: "".to_string(),
                license: "".to_string(),
            }, &output, &pb).unwrap();
        }
        Some(Commands::SymbolsHash { input, output }) => {
            pipeline::run_symbols_hash_pipeline(input, output); 
        }
        Some(Commands::SymbolsCompare { from, input, results, language }) => {
            hash::find_most_similar(input, results, from, &language);
        }
        Some(Commands::SymbolsCompareAll { input, repositories, output }) => {
            pipeline::run_symbols_compare_all_pipeline(input, repositories, output);
        }
    }
}