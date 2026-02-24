pub fn get_input(file_path: &str) -> Result<Vec<InputData>, Box<dyn std::error::Error>> {
	let input = read_input(file_path)?;
	Ok(process_input(&input))
}

fn read_input(file_path: &str) -> Result<String, std::io::Error> {
	std::fs::read_to_string(file_path)
}

pub struct InputData {
	pub author: String,
	pub name: String,
}

fn process_input(input: &str) -> Vec<InputData> {
	input.lines().filter_map(|line| {
		let parts: Vec<&str> = line.split('/').map(|s| s.trim()).collect();
		if parts.len() == 2 {
			Some(InputData {
				author: parts[0].to_string(),
				name: parts[1].to_string(),
			})
		} else {
			None
		}
	}).collect()
}