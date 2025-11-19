import os
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
import numpy as np
from itertools import combinations

OWNER_NAME = "openai"
REPO_NAME = "codex"
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

## Correlation of language changes between pairs of languages
diff_df = numeric_df.diff().dropna(how="all")
if diff_df.empty:
    raise RuntimeError("No changes (diff) to plot for any language.")

non_constant_cols = [
    col for col in diff_df.columns
    if diff_df[col].replace(0, np.nan).notna().any()
]
diff_df = diff_df[non_constant_cols]

if diff_df.shape[1] < 2:
    raise RuntimeError("Need at least two languages with non-zero changes to plot pairs.")

dates = diff_df.index.to_pydatetime()
dates_num_all = mdates.date2num(dates)
cmap = plt.get_cmap("viridis")

def slugify(name: str) -> str:
    """Create a filesystem-safe slug from a column / language name."""
    name = name.replace("+", "p")
    return "".join(ch if ch.isalnum() else "" for ch in str(name))

def plot(x, y, x_fit, y_fit, pair_changes):
    fig, ax = plt.subplots(figsize=(10, 7))
    dates_pair = pair_changes.index.to_pydatetime()
    dates_num = mdates.date2num(dates_pair)

    sc = ax.scatter(
        x,
        y,
        c=dates_num,
        cmap=cmap,
        edgecolor="k",
        alpha=0.8,
    )

    # Plot trend line
    ax.plot(x_fit, y_fit, linewidth=1.5, label="trend", zorder=1)

    ax.set_xlabel(f"Change in {col_x} LOC (delta)")
    ax.set_ylabel(f"Change in {col_y} LOC (delta)")
    ax.set_title(f"{OWNER_NAME}/{REPO_NAME}: {col_y} vs {col_x} LOC change per time point")
    ax.grid(True)
    ax.legend()

    # Colorbar with human-readable date ticks
    cbar = fig.colorbar(sc, ax=ax)
    ticks = np.linspace(dates_num.min(), dates_num.max(), min(6, len(dates_pair)))
    cbar.set_ticks(ticks)
    cbar.set_ticklabels([mdates.num2date(t).strftime("%Y-%m-%d") for t in ticks])
    cbar.set_label("date")

    plt.tight_layout()

    filename = f"{OWNER_NAME}_{REPO_NAME}/{slugify(col_x)}_vs_{slugify(col_y)}_scatter.png"
    plt.savefig(os.path.join(RESULTS_DIR, filename))
    plt.close(fig)

for col_x, col_y in combinations(diff_df.columns, 2):
    pair_changes = diff_df[[col_x, col_y]].copy()

    mask = (pair_changes[col_x] != 0) & (pair_changes[col_y] != 0)
    pair_changes = pair_changes[mask]

    if pair_changes.shape[0] < 2:
        continue

    x = pair_changes[col_x].values
    y = pair_changes[col_y].values

    slope, intercept = np.polyfit(x, y, 1)
    x_fit = np.linspace(x.min(), x.max(), 200)
    y_fit = slope * x_fit + intercept
    avg_x = np.mean(x)
    avg_y = np.mean(y)
    name_w = diff_df.columns.map(len).max()
    num_w = 9
    points_w = 5

    col_x_s = str(col_x)[:name_w].ljust(name_w)
    col_y_s = str(col_y)[:name_w].ljust(name_w)

    coeff = slope * (np.std(x) / np.std(y))
    relative_size = abs(avg_x) / abs(avg_y) if (avg_y != 0) else float('inf')

    print(
        f"{col_x_s} vs {col_y_s} | "
        f"coeff={slope * (np.std(x) / np.std(y)):<+0.4f} | "
        f"mean {col_x_s}={avg_x:>{num_w}.2f} | mean {col_y_s}={avg_y:>{num_w}.2f} | "
        f"rel_size={relative_size:>{num_w}.4f} | "
        f"points={len(x):>{points_w}d}"
    )
    if col_x == "Rust" or col_y == "Rust":
        plot(x, y, x_fit, y_fit, pair_changes)
    if coeff < -0.5 and len(x) >= 5 and relative_size > 0.1 and abs(avg_y) > 100 and abs(avg_x) > 100:
        print("  --> Significant negative correlation detected!")
        plot(x, y, x_fit, y_fit, pair_changes)
