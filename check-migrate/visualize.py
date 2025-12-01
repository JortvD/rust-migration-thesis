import os
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
import numpy as np

OWNER_NAME = "servo"
REPO_NAME = "servo"
RESULTS_DIR = "results"

os.makedirs(RESULTS_DIR, exist_ok=True)
os.makedirs(os.path.join(RESULTS_DIR, f"{OWNER_NAME}_{REPO_NAME}"), exist_ok=True)

data = pd.read_csv(os.path.join(RESULTS_DIR, f"{OWNER_NAME}_{REPO_NAME}.csv"))

data["date"] = pd.to_datetime(data["date"], errors="coerce")
data = data.sort_values("date").set_index("date")

numeric_df = data.drop(columns=["hash"], errors="ignore").select_dtypes(include="number")

if numeric_df.empty:
    raise RuntimeError("No numeric columns to plot after dropping 'hash'.")

## Stacked area plot of language percentages over time
row_sums = numeric_df.sum(axis=1)
valid_rows = row_sums.replace(0, np.nan).notna()
percent_df = numeric_df[valid_rows].div(row_sums[valid_rows], axis=0) * 100

if percent_df.empty:
    raise RuntimeError("No valid rows with non-zero totals for percentage plot.")

dates = percent_df.index.to_pydatetime()
dates_num = mdates.date2num(dates)

n_lang = percent_df.shape[1]
cmap = plt.get_cmap("tab20")
colors = [cmap(i / max(1, n_lang - 1)) for i in range(n_lang)]

fig, ax = plt.subplots(figsize=(12, 6))
ax.stackplot(dates_num, percent_df.T.values, labels=percent_df.columns, colors=colors, alpha=0.95)

locator = mdates.AutoDateLocator()
formatter = mdates.ConciseDateFormatter(locator)
ax.xaxis.set_major_locator(locator)
ax.xaxis.set_major_formatter(formatter)
ax.set_xlim(dates_num.min(), dates_num.max())

ax.set_ylim(0, 100)
ax.set_xlabel("date")
ax.set_ylabel("Percentage of total LOC")
ax.set_title(f"Language share over time for {OWNER_NAME}/{REPO_NAME}")
ax.legend(title="Language", bbox_to_anchor=(1.01, 1), loc="upper left")
ax.grid(axis="y")

plt.xticks(rotation=45, ha="right")
plt.tight_layout()
plt.savefig(os.path.join(RESULTS_DIR, f"{OWNER_NAME}_{REPO_NAME}/stacked_percentages.png"))
plt.close(fig)
