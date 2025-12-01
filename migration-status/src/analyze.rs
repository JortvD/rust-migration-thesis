use bio::io::common;
use tokei::LanguageType;

use crate::gather;

pub fn tokei_migration_test(result: gather::TokeiStatistics) -> bool {
    let steps = result.steps;
    let num_steps = steps.len();

	if num_steps < 2 {
		return false;
	}

	let first_step = &steps[0];
	let last_step = &steps[num_steps - 1];

	let total_first: usize = first_step.languages.values().map(|stat| stat.code).sum();
	let total_last: usize = last_step.languages.values().map(|stat| stat.code).sum();

	let rust_first_percent = if let Some(rust_stat) = first_step.languages.get(&LanguageType::Rust) {
		rust_stat.code as f64 / total_first as f64
	} else {
		0.0
	};

	let rust_last_percent = if let Some(rust_stat) = last_step.languages.get(&LanguageType::Rust) {
		rust_stat.code as f64 / total_last as f64
	} else {
		0.0
	};

	rust_first_percent < 0.05 && rust_last_percent > 0.2
}

pub fn cargo_migration_test(result: &gather::CargoStatistics) -> bool {
	let steps = &result.steps;
	let num_steps = steps.len();

	if num_steps < 2 {
		return false;
	}

	let first_step = &steps[0];
	let last_step = &steps[num_steps - 1];

	let first_count = first_step.num_cargo_toml;
	let last_count = last_step.num_cargo_toml;

	first_count == 0 && last_count >= 1
}

pub fn cargo_find_before_after(result: &gather::CargoStatistics) -> (String, String) {
	let mut before = String::new();
	let mut after = String::new();

	for step in &result.steps {
		if step.num_cargo_toml == 0 {
			before = step.commit_id.clone();
		} else {
			after = step.commit_id.clone();
		}
	}

	(before, after)
}

pub fn matches_migration_test(result: gather::MatchesStatistics) -> bool {
	let symbols_before = result.before.len();
	let common_symbols = result.common_symbols.len();

	common_symbols as f64 / symbols_before as f64 > 0.2
}