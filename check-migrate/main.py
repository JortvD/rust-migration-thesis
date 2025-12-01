import subprocess
from git import Repo
from datetime import datetime
import numpy as np
# import ghlinguist as ghl
import shutil
import pandas as pd
import os

RESULTS_DIR = "results"
OWNER_NAME = "dani-garcia"
REPO_NAME = "vaultwarden"
TEMP_DIR = f"temp/{OWNER_NAME}_{REPO_NAME}"
NUM_COMMITS = 100

if os.path.exists(TEMP_DIR):
	import shutil
	shutil.rmtree(TEMP_DIR)

EXE = shutil.which("github-linguist")

repo = Repo.clone_from(f"https://github.com/{OWNER_NAME}/{REPO_NAME}.git", TEMP_DIR)
# repo = Repo(TEMP_DIR)

main_branch = repo.heads.master if "master" in repo.heads else repo.heads.main
print(f"Analyzing branch: {main_branch.name}")

commits = list(repo.iter_commits(main_branch))

if len(commits) > NUM_COMMITS:
	indices = np.linspace(0, len(commits) - 1, NUM_COMMITS, dtype=int)
	selected_commits = [commits[i] for i in indices]
else:
	selected_commits = commits

selected_commits.reverse()  # analyze from oldest to newest

data = pd.DataFrame(columns=["hash", "date"])
languages = []
start_total = datetime.now()
for i, commit in enumerate(selected_commits):
	start_time = datetime.now()
	
	try:
		repo.git.checkout(commit.hexsha)
	except Exception:
		repo.git.reset('--hard')
		repo.git.clean('-fdx')
		repo.git.checkout('-f', commit.hexsha)

	ret = subprocess.check_output([EXE, TEMP_DIR], text=True).split("\n")

	results = []
	for line in ret:
		L = line.split()
		if not L:  # EOF
			break
		results.append((L[-1], L[1]))

	for lang, pct in results:
		if lang not in languages:
			languages.append(lang)
			data[lang] = 0.0
		
	data.loc[len(data)] = {
		"hash": commit.hexsha,
		"date": datetime.fromtimestamp(commit.committed_date).strftime("%Y-%m-%d")
	}

	for lang in languages:
		if lang in dict(results):
			data.at[len(data) - 1, lang] = float(dict(results)[lang])
		else:
			data.at[len(data) - 1, lang] = 0.0

	duration = datetime.now() - start_time
	print(f"[{i+1:03d}/{len(selected_commits)}] Analyzed commit {commit.hexsha[:8]} from {datetime.fromtimestamp(commit.committed_date).strftime('%Y-%m-%d')} in {duration.total_seconds() * 1000} ms.")

duration_total = datetime.now() - start_total
print(f"Total analysis time: {duration_total.total_seconds() * 1000} ms.")

if not os.path.exists(RESULTS_DIR):
	os.makedirs(RESULTS_DIR)
data.to_csv(f"{RESULTS_DIR}/{OWNER_NAME}_{REPO_NAME}.csv", index=False)