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
            default_value = "results/analysis/",
        )]
        output: String,
    },
    Single {
        owner: String,
        repo: String,

        #[arg(
            long,
            help = "Temporary directory for cloning",
            default_value = "temp",
        )]
        temp_dir: String,

        #[arg(
            long,
            help = "Number of commits to analyze",
            default_value_t = 100,
        )]
        num_commits: usize,
    },
    Parse {
        path: String,
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
        Some(Commands::Parse {
            path,
        }) => {
            let path = std::path::Path::new(path);
            if let Some(symbols) = code::extract_symbols_for_file(path) {
                println!("Found {} symbols:", symbols.len());
                for symbol in symbols {
                    println!("{:?}", symbol);
                }
            } else {
                println!("No symbols found.");
            }
        }
        Some(Commands::Single {
            owner,
            repo,
            temp_dir,
            num_commits,
        }) => {
            let writer = &mut std::io::stdout();
            let gather_result = gather::gather_repository_statistics(
                owner,
                repo,
                temp_dir,
                *num_commits,
                writer,
            );

            match gather_result {
                Ok(stats) => {
                    let status = analyze::check_migration_status(stats, writer);

                    println!("Result={:?}", status);
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            }
        }
    }
}