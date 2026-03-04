use std::{fs::{File, remove_file}, io::Write, time::Duration};

use indicatif::ProgressBar;

use reqwest;

const METRICS: &str = "files,classes,functions,lines,statements,new_lines,ncloc,comment_lines,comment_lines_density,software_quality_security_issues,new_software_quality_security_issues,software_quality_security_remediation_effort,new_software_quality_security_remediation_effort,software_quality_reliability_issues,new_software_quality_reliability_issues,software_quality_reliability_remediation_effort,new_software_quality_reliability_remediation_effort,software_quality_maintainability_issues,new_software_quality_maintainability_issues,software_quality_maintainability_remediation_effort,new_software_quality_maintainability_remediation_effort,software_quality_maintainability_debt_ratio,new_software_quality_maintainability_debt_ratio,security_hotspots,new_security_hotspots,duplicated_lines_density,new_duplicated_lines_density,duplicated_lines,new_duplicated_lines,duplicated_blocks,new_duplicated_blocks,duplicated_files,complexity,cognitive_complexity,violations,new_violations,blocker_violations,critical_violations,major_violations,minor_violations,info_violations";
const API_URL: &str = "http://49.13.5.68:9000";
const PAGE_SIZE: usize = 500;

pub struct Project {
    pub name: String,
    pub token: String,
    pub results_folder: String,
}

impl Project {
    pub fn create(name: &str, folder: &str) -> Result<Project, Box<dyn std::error::Error>> {
        let client = reqwest::blocking::Client::new();
        let response = client.post(format!("{}/api/projects/create?name={}&project={}", API_URL, name, name))
            .bearer_auth(std::env::var("SONARQUBE_KEY")?)
            .send()?;

        Ok(Project {
            name: name.to_string(),
            token: std::env::var("SONARQUBE_KEY")?,
            results_folder: folder.to_string(),
        })
    }

    pub fn run_analysis(&self, repo_dir: &str, index: usize) -> Result<u64, Box<dyn std::error::Error>> {
        let properties_file = format!("{}/sonar-project.properties", repo_dir);
		let mut properties_file_wtr = File::create(&properties_file)?;
        properties_file_wtr.write_all(format!("sonar.projectKey={}\n", self.name).as_bytes())?;
        properties_file_wtr.write_all(b"sonar.exclusions=**/*test*/**/*,**/*test*\n")?;
        properties_file_wtr.write_all(format!("sonar.projectVersion=v{}\n", index + 1).as_bytes())?;

        if std::path::Path::new(&format!("{}/pom.xml", repo_dir)).exists() {
            let output = std::process::Command::new("mvn")
                .arg("compile")
                .arg("-fn")
                .current_dir(repo_dir)
                .output()?;
            let log_file_path = format!("{}/{}_mvn_logs.txt", self.results_folder, index);
            let mut log_file = File::create(&log_file_path)?;
            log_file.write_all(format!("{}", String::from_utf8_lossy(&output.stdout)).as_bytes())?;

            let error_file_path = format!("{}/{}_mvn_errors.txt", self.results_folder, index);
            let mut error_file = File::create(&error_file_path)?;
            error_file.write_all(format!("{}", String::from_utf8_lossy(&output.stderr)).as_bytes())?;

            properties_file_wtr.write_all(b"sonar.java.binaries=**/target/classes\n")?;
        }

        let output = std::process::Command::new("docker")
            .arg("run")
            .arg("--rm")
            .arg("-e")
            .arg("SONAR_HOST_URL=http://49.13.5.68:9000")
            .arg("-e")
            .arg(format!("SONAR_TOKEN={}", self.token))
            .arg("-v")
            .arg("./:/usr/src")
            .arg("sonarsource/sonar-scanner-cli")
            .current_dir(repo_dir)
            .output()?;

        let log_file_path = format!("{}/{}_logs.txt", self.results_folder, index);
        let mut log_file = File::create(&log_file_path)?;
        log_file.write_all(format!("{}", String::from_utf8_lossy(&output.stdout)).as_bytes())?;

        let error_file_path = format!("{}/{}_errors.txt", self.results_folder, index);
        let mut error_file = File::create(&error_file_path)?;
        error_file.write_all(format!("{}", String::from_utf8_lossy(&output.stderr)).as_bytes())?;

        remove_file(&properties_file)?;

        Ok(output.status.code().unwrap_or(0) as u64)
    }

    pub fn get_results(&self, index: usize, bar: &ProgressBar) -> Result<u64, Box<dyn std::error::Error>> {
        let client = reqwest::blocking::Client::new();
        let mut page: u64 = 1;
        let mut all_data = serde_json::Map::new();
        all_data.insert("components".to_string(), serde_json::Value::Array(Vec::new()));
        let mut total: u64 = 0;
        loop {
            let result = client.get(format!("{}/api/measures/component_tree?component={}&metricKeys={}&ps={}&p={}", API_URL, self.name, METRICS, PAGE_SIZE, page))
                .bearer_auth(&self.token)
                .timeout(Duration::from_secs(10))
                .send();

            if let Err(e) = result {
                bar.set_message(format!("{} [{}/{}] Failed to retrieve results for page {}: {}, retrying...", self.name, index + 1, PAGE_SIZE, page, e));
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            let response = result.unwrap();

            if !response.status().is_success() {
                bar.set_message(format!("{} [{}/{}] Failed to retrieve results for page {}: HTTP {}, retrying...", self.name, index + 1, PAGE_SIZE, page, response.status()));
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }

            let json: serde_json::Value = response.json()?;
            total = json["paging"]["total"].as_u64().unwrap_or(0);
            if page == 1 {
                all_data.insert("base".to_string(), json["baseComponent"].clone());
            }
            bar.set_message(format!("{} [{}/{}] Retrieved page {}, checking next...", self.name, index + 1, PAGE_SIZE, page));
            if let Some(components) = json["components"].as_array() {
                // println!("Page {} has {} items", page, components.len());
                if let Some(all_components) = all_data.get_mut("components").and_then(|v| v.as_array_mut()) {
                    all_components.extend(components.clone());
                }
            }
            if total < (PAGE_SIZE as u64) * page {
                break;
            }
            page += 1;
        }
        let output_file = format!("{}/{}.json", self.results_folder, index);
        let mut output_file_wtr = File::create(&output_file)?;
        serde_json::to_writer_pretty(&mut output_file_wtr, &all_data)?;
        Ok(total)
    }

    pub fn get_activity_count(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let response = reqwest::blocking::Client::new().get(format!("{}/api/project_analyses/search?project={}&ps=500", API_URL, self.name))
            .bearer_auth(&self.token)
            .timeout(Duration::from_secs(10))
            .send()?;
        let json: serde_json::Value = response.json()?;
        Ok(json["paging"]["total"].as_u64().unwrap_or(0))
    }

    pub fn delete(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::blocking::Client::new();
        let response = client.post(format!("{}/api/projects/delete?project={}", API_URL, self.name))
            .bearer_auth(&self.token)
            .send()?;
        // println!("Response: {}", response.status());
        Ok(())
    }
}
