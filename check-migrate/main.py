from git import Repo
from datetime import datetime
import numpy as np
import ghlinguist as ghl
import pandas as pd
import os

TEMP_DIR = "temp"
RESULTS_DIR = "results"
OWNER_NAME = "topjohnwu"
REPO_NAME = "Magisk"
NUM_COMMITS = 10

repo = Repo.clone_from(f"https://github.com/{OWNER_NAME}/{REPO_NAME}.git", TEMP_DIR)
# repo = Repo(TEMP_DIR)

main_branch = repo.heads.master
print(f"Analyzing branch: {main_branch.name}")

commits = list(repo.iter_commits(main_branch))

if len(commits) > NUM_COMMITS:
	indices = np.linspace(0, len(commits) - 1, NUM_COMMITS, dtype=int)
	selected_commits = [commits[i] for i in indices]
else:
	selected_commits = commits

data = pd.DataFrame(columns=["hash", "date", "folder", "languages"])

for commit in selected_commits:
	start_time = datetime.now()
	repo.git.checkout(commit.hexsha)

	folders = [""]
	for item in os.listdir(TEMP_DIR):
		if item.startswith(".") or not os.path.isdir(os.path.join(TEMP_DIR, item)):
			continue
		folders.append(item)
		for subitem in os.listdir(os.path.join(TEMP_DIR, item)):
			if subitem.startswith(".") or not os.path.isdir(os.path.join(TEMP_DIR, item, subitem)):
				continue
			folders.append(f"{item}/{subitem}")
		
	for folder in folders:
		data = data.append({
			"hash": commit.hexsha,
			"date": datetime.fromtimestamp(commit.committed_date).strftime("%Y-%m-%d"),
			"folder": folder,
			"languages": ghl.linguist(TEMP_DIR + "/" + folder)
		})

	duration = datetime.now() - start_time
	print(f"Analyzed commit {commit.hexsha[:8]} from {datetime.fromtimestamp(commit.committed_date).strftime('%Y-%m-%d')} with {len(folders)} folders in {duration.total_seconds() * 1000} ms.")

if not os.path.exists(RESULTS_DIR):
	os.makedirs(RESULTS_DIR)
data.to_csv(f"{RESULTS_DIR}/{OWNER_NAME}_{REPO_NAME}.csv", index=False)