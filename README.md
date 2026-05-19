# rust-migration-thesis

This repository is the replication package for Jort van Driel's Master's thesis: **"On Software Projects that Migrate to Rust"** (TU Delft, 2026).

The thesis uses a mixed-methods approach to study open-source software projects that migrated to Rust. It identifies 285 such migrations using a novel identifier-tracking technique, then combines developer interviews with 38 participants with a quantitative analysis across 22 software quality metrics.

> **Note on privacy**: interview recordings, consent forms, and any other files that could identify participants are not included in this repository.

---

## Repository layout

```
migration-status/   # Rust tool — all migration detection steps (RQ1)
github/             # Rust tool — collects GitHub data for quality metrics (RQ3)
code/               # Rust tool — code quality metrics via RCA, Truck Factor, and SZZ (RQ3)
sonar/              # Rust tool — SonarQube metric collection (RQ3)
common/             # Shared Rust utilities (bare-clone helpers, input parsing)
analysis/           # Python notebooks — all result processing and figures
  data/             #   static input files (characteristics, phases, labels)
  input/            #   migration phase definitions per project
  metrics/          #   notebooks that compute per-metric results from raw tool output
  projects/         #   notebooks that describe the migration dataset (RQ1)
  results/          #   generated CSVs consumed by the analysis notebooks
requirements.txt    # Python dependencies
```

---

## Prerequisites

| Tool | Purpose |
|------|---------|
| Rust (stable, 2024 edition) | build all four Rust tools |
| Python ≥ 3.11 | run the analysis notebooks |
| Jupyter | open `.ipynb` files |
| A GitHub Personal Access Token (PAT) | stored in a `.env` file as `GITHUB_PAT=...` |
| SonarQube instance | required to re-run the `sonar` tool |

Install Python dependencies:

```bash
pip install -r requirements.txt
pip install polars scipy powerlaw  # additional dependencies used in the notebooks
```

---

## Step-by-step replication

### Step 1: Detect migrations (RQ1)

All migration detection is handled by the `migration-status` tool. This includes collecting the initial set of repositories from GitHub, running the in-repository identifier analysis, and performing the cross-repository similarity search.

#### 1a. Collect repositories from GitHub

This step queries the GitHub Search API and the `/languages` endpoint to build a dataset of all popular repositories with at least 1% Rust code.

```bash
cd migration-status
echo "GITHUB_PAT=ghp_yourtoken" > .env
cargo run --release -- symbols-collect --min-stars 300
```

**Output**: `results/all_repositories.csv` — all GitHub repositories with at least 300 stars, with their metadata and language breakdown.

#### 1b. In-repository detection

This step analyses the commit history of each repository to find identifier *movement* and *overlap* between language pairs. For each repository, 100 commits are sampled evenly across the full history. The AST of each source file at each sampled commit is parsed using `tree-sitter`, and identifiers are extracted and normalised (lowercased, underscores removed, minimum length of four characters). Movement is detected when an identifier is present in a legacy language and later appears exclusively in Rust.
Note that here we have used a script to reduce our set of all repositories to all repositories over 300 stars.

```bash
cargo run --release -- analysis \
  --input  ../analysis/data/rust_repositories.csv \
  --output results/analysis
```

**Input**: a CSV of repositories filtered to those containing Rust code.  
**Output**: `results/analysis/<owner>_<repo>/` — compressed archives of identifier data per sampled commit.

#### 1c. Cross-repository detection

This step detects migrations where the Rust version was developed in a separate repository. Identifiers are collected from the latest commit of every repository, hashed into a compact MinHash signature using the SuperMinHash algorithm (1024 hashes per signature), and then all Rust repositories are compared against all non-Rust repositories to find the closest matches by Jaccard similarity.

```bash
# Collect identifier snapshots for all repositories
cargo run --release -- symbols \
  --input  ../analysis/data/rust_repositories.csv \
  --output results/symbols

# Compute MinHash signatures
cargo run --release -- symbols-hash \
  --input  results/symbols \
  --output results/symbols_hash.bin

# Find the closest match for each Rust repository
cargo run --release -- symbols-compare-all \
  --input        results/symbols_hash_5.bin \
  --repositories results/repositories_correct.csv \
  --output       results/final_symbols_compare.csv
```

**Output**: `results/final_symbols_compare.csv` — Jaccard similarity scores between repository pairs.

#### 1d. Manual labelling

Candidates flagged by the automated steps above are verified manually by checking the repository's commit history, pull requests, and documentation for explicit references to migration. The results are stored in:

- `results/labelling/train.csv` — labelled examples used during threshold calibration
- `results/labelling/validate.csv` — held-out validation set
- `results/final_analysis.csv` — scored list of all candidates
- `analysis/data/migration_labels.csv` — final verification status for each candidate (`Full`, `Partial`, `Sep`, `Subs`, or `No`)

---

### Step 2: Collect quality metric data (RQ3)

#### 2a. GitHub data (issues, releases, security advisories)

The `github` tool collects the data needed for the issue-based and distribution metrics. For each project, it fetches all issues (with their event timelines), all releases, and all security advisories from the GitHub API.

```bash
cd github
echo "GITHUB_PAT=ghp_yourtoken" > .env
# Pass the output folder as the first argument
cargo run --release -- results
```

**Input**: `github/input.txt` — one `owner/repo` per line (use `owner/repo/original_owner/original_repo` for cross-repository migration pairs).  
**Output**: `github/results/<owner>_<repo>/` — JSON files for each issue, issue timeline, release, and advisory. Also writes `github/results/characteristics.csv` with stars, forks, size, and Rust percentage for each project and its original.

The following metrics are derived from this data in the analysis notebooks: defect issue count, defect issue resolution time, crash issue count, distribution platform count, mean release size, and vulnerability count.

#### 2b. Static code analysis (RCA, Truck Factor, SZZ)

The `code` tool clones each repository and samples up to 250 commits evenly across the project's history. For each sampled commit, it runs three analyses:

- **rust-code-analysis** (an extended fork) — extracts LOC, cyclomatic complexity, cognitive complexity, documentation density, test density, unsafe code density, and the assertions-to-complexity ratio.
- **truck-facto-rs** — computes the Truck Factor and Gini coefficient to measure contributor concentration (with and without time-decay weighting).
- **szz-rs** — identifies bug-inducing commits using the SZZ algorithm, from which the time between introducing and fixing a bug is derived.

```bash
cd code
cargo run --release
```

**Input**: `code/input.txt` — one `owner/repo` per line.  
**Output**: `code/results/<owner>_<repo>/` — gzipped JSON files per sampled commit.

#### 2c. SonarQube metrics

The `sonar` tool clones each repository, checks out 250 sampled commits, submits each to a running SonarQube instance, and downloads the resulting metrics.

```bash
cd sonar
# Requires SONAR_URL and SONAR_TOKEN in .env
cargo run --release
```

**Input**: `sonar/input.txt`  
**Output**: `sonar/results/<owner>_<repo>/` — JSON files per sampled commit with metrics for code duplication, technical debt, reliability debt, and security debt.

---

### Step 3: Analyse results (Python notebooks)

All notebooks are in `analysis/`. Run them with Jupyter from that directory so that relative paths resolve correctly.

#### Project-level notebooks (`analysis/projects/`)

These notebooks describe the migration dataset and answer RQ1.

| Notebook | Contents |
|----------|----------|
| `all.ipynb` | overall dataset statistics (173,806 repos → 5,786 Rust repos → migration set) |
| `in_repo.ipynb` | in-repository detection results and threshold calibration |
| `cross_repo.ipynb` | cross-repository Jaccard similarity results |
| `migrations.ipynb` | characterisation of the final migration set by language, domain, and popularity |
| `characteristics.ipynb` | K-means cluster labels for stars, forks, size, and Rust percentage |

Key input files used by these notebooks:

- `data/all_repositories.csv` — all repositories with at least 300 stars
- `data/rust_repositories.csv` — subset with at least 1% Rust code
- `data/migration_labels.csv` — manual verification labels
- `data/characteristics.csv` / `data/verified_characteristics.csv` — manually verified project metadata

#### Metric notebooks (`analysis/metrics/`)

Each notebook reads raw output from the Rust tools, aggregates measurements per migration phase (`pre`, `during`, `post`), and writes a timestamped CSV to `analysis/results/`.

| Notebook | Metrics computed |
|----------|-----------------|
| `rca.ipynb` | LOC, cyclomatic complexity (total and per unit), cognitive complexity (total and per unit), documentation density, test density, unsafe code density, assertions-to-complexity ratio |
| `sonarqube.ipynb` | code duplication, technical debt density, reliability debt density, security debt density, security issues |
| `truck.ipynb` | Truck Factor, Gini coefficient, and author count (with and without DOA time-decay) |
| `commits.ipynb` | code churn per month |
| `szz.ipynb` | time from bug-inducing commit to fix commit |
| `issues.ipynb` | defect issue count, defect issue resolution time, crash issue count |
| `distribution.ipynb` | distribution platform count, mean release size |
| `vulnerabilities.ipynb` | vulnerability count |

The key input file for all metric notebooks is `analysis/input/phases.csv`, which defines the start and end dates for the `pre`, `during`, and `post` phases of each project.

**Output**: `analysis/results/<metric_name>-<timestamp>.csv` — one row per (project, phase, date, value). The most recent file for each metric is used in the statistical analysis.

#### Statistical analysis

`analysis/plot.ipynb` and `analysis/table.ipynb` load the per-metric CSVs, run the Wilcoxon signed-rank test comparing pre/during/post phases, and produce the figures and tables presented in the thesis.

---

## Key result files

| File | Description |
|------|-------------|
| `migration-status/results/repositories_with_rust_over_250_stars.csv` | all GitHub repos with ≥300 stars and ≥1% Rust code |
| `migration-status/results/final_analysis.csv` | scored migration candidates with movement, overlap, and Jaccard scores |
| `migration-status/results/final_symbols_compare.csv` | cross-repository Jaccard similarity scores |
| `migration-status/results/labelling/train.csv` / `validate.csv` | hand-labelled calibration and validation sets |
| `analysis/data/migration_labels.csv` | verification status for all candidates in the Rust set |
| `analysis/data/verified_characteristics.csv` | final migration set with domain, language, and cluster labels |
| `analysis/char_groups.csv` | K-means cluster tier (Low/Medium/High) per project for stars, forks, size, and Rust% |
| `analysis/input/phases.csv` | migration phase date ranges per project |
| `analysis/results/<metric>-<timestamp>.csv` | per-metric time-series data (one file per metric per run; the most recent is used) |

---

## Related repositories

The following repositories were developed as part of this thesis:

- **[truck-facto-rs](https://github.com/JortvD/truck-facto-rs)** — Rust library for computing the Truck Factor and Gini coefficient, with optional time-decay weighting.
- **[szz-rs](https://github.com/JortvD/szz-rs)** — Rust implementation of the SZZ algorithm for identifying bug-inducing commits.
- **[rust-code-analysis (fork)](https://github.com/JortvD/rust-code-analysis/)** — Extended version of Mozilla's Rust-Code-Analysis with support for a broader set of programming languages.
