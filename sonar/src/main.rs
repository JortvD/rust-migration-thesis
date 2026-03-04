use std::collections::HashMap;

use dotenv::dotenv;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::input::get_input;
use crate::pipeline::{MAX_SAMPLES, run_pipeline};

mod input;
mod pipeline;
mod project;
mod repository;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let lines = get_input("input.txt")?;

    rayon::ThreadPoolBuilder::new()
        .num_threads(11)
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

    for line in lines {
        let current_thread_id = rayon::current_thread_index().unwrap_or(0);
		let bar: &ProgressBar = handles.get(&current_thread_id).unwrap();
        let result = run_pipeline(&line, bar, &overall_bar);
        
        if let Err(e) = result {
            eprintln!("Error running pipeline: {:?}", e);
        }
    }

    Ok(())
}