use std::collections::HashMap;

use dotenv::dotenv;

use common::input::get_input;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

mod pipeline;
mod collect;
mod truck;
mod szz;
mod commits;

use crate::pipeline::{MAX_SAMPLES, run_pipeline};

fn main() {
    // let components = collect::collect_repository("/home/jortvd/Documents/GitKraken/rust-migration-thesis/sonar/temp/amir20_dtop").expect("Failed to collect repository");
    // collect::save_components(&std::path::Path::new("components.json"), &components).expect("Failed to save components");
    // let path = Path::new("./test.hs");
    // let source = rust_code_analysis::read_file(path).expect("Failed to read file");
    // let parser = rust_code_analysis::HaskellParser::new(source, &path, None);

    // let result = rust_code_analysis::metrics(&parser, &path).unwrap();

    // println!("Metrics: {:#?}", result);
    dotenv().ok();
    let lines = get_input("input.txt").expect("Failed to read input");

    rayon::ThreadPoolBuilder::new()
        .num_threads(12)
        .build_global()
        .expect("Failed to create thread pool");

    let mp = MultiProgress::new();
	let overall_bar = mp.add(ProgressBar::new((lines.len() * MAX_SAMPLES) as u64));
    overall_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "[{elapsed_precise}] [{bar:100.cyan/blue}] {pos}/{len} ({eta})"
            )
            .expect("Failed to create template")
            .progress_chars("#>-"),
    );

	let mut handles: HashMap<usize, ProgressBar> = HashMap::new();
	for thread in 0..rayon::current_num_threads() {
		let pb = mp.add(ProgressBar::new(0));
		pb.set_style(
			ProgressStyle::default_spinner()
				.template(
					&format!("[{{elapsed_precise}}] [Thread {}] {{spinner}} {{msg}}", thread)
				)
				.expect("Failed to create template")
				.tick_chars("/|\\- "),
		);
		pb.set_message("Starting...");
		handles.insert(thread, pb);
	}

    lines.into_par_iter().for_each(|line| {
        let current_thread_id = rayon::current_thread_index().unwrap_or(0);
		let bar: &ProgressBar = handles.get(&current_thread_id).unwrap();
        let result = run_pipeline(&line, bar, &overall_bar);
        
        if let Err(e) = result {
            eprintln!("Error running pipeline: {:?}", e);
        }
    });
}
