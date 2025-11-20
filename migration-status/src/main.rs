use clap::{Parser, Subcommand};

mod analyze;
mod repository;
mod plots;

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
        division_plot: bool,
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
        toml_plot: bool,
    },
    Matches {
        owner: String,
        repo: String,

        commit1_hash: String,
        commit2_hash: String,

        #[arg(short, long, help = "Temporary directory for analysis", default_value = "temp")]
        temp_dir: String,
    }
}

fn main() {
    let args = Args::parse();

    match &args.command {
        Some(Commands::Tokei {
            owner,
            repo,
            temp_dir,
            num_commits,
            division_plot,
        }) => {
            match analyze::tokei(owner, repo, temp_dir, *num_commits) {
                Ok(result) => {
                    if *division_plot {
                        if let Err(e) = plots::plot_language_division(&result, owner, repo) {
                            eprintln!("Failed to generate division plot: {:?}", e);
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
            toml_plot,
        }) => {
            match analyze::cargo(owner, repo, temp_dir, *num_commits) {
                Ok(result) => {
                    if *toml_plot {
                        if let Err(e) = plots::plot_toml_amount(&result, owner, repo) {
                            eprintln!("Failed to generate Cargo.toml amount plot: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Cargo analysis failed: {:?}", e);
                }
            }
        }
        Some(Commands::Matches { owner, repo, commit1_hash, commit2_hash, temp_dir }) => {
            match analyze::matches(owner, repo, temp_dir, commit1_hash, commit2_hash) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Matching substrings analysis failed: {:?}", e);
                }
            }
        }
        None => {
            println!("No command provided. Use --help for more information.");
        }
    }
}
