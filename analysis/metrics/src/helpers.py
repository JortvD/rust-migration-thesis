from pathlib import Path
import random
import isal.igzip as gzip

from tqdm import tqdm
import orjson
from datetime import datetime, timezone
import polars as pl
import multiprocessing

def get_project_folders(results_folder: str) -> list[Path]:
    return sorted([f for f in Path(results_folder).glob("*") if f.is_dir()])

def normalize_project(name: str) -> str:
    if "/" in name:
        return name
    
    return name.replace("_", "/", 1)

def get_zip_json_data(file: Path):
    with gzip.open(file, "rb") as z:
        return orjson.loads(z.read())
    
def get_json_data(file: Path):
    with open(file, "rb") as f:
        return orjson.loads(f.read())
    
def get_timestamps(folder: Path) -> list[datetime]:
    commits_file = Path(folder) / "commits.csv"
    df = pl.read_csv(
        commits_file,
        has_header=True,
        quote_char=None,
        truncate_ragged_lines=True,
        ignore_errors=True
    )
    return [
        datetime.strptime(x, "%Y-%m-%d %H:%M:%S %Z").replace(tzinfo=timezone.utc)
        for x in df.get_column("commit_time").to_list()
    ]

def iterate_parallel(func, folder: str, matches: str, metrics) -> list:
    items = []
    for project_folder in get_project_folders(folder):
        timestamps = get_timestamps(project_folder)
        for file in sorted(project_folder.glob(matches)):
            it = int(file.name.split(".")[0])
            items.append((file.parent.name, file, timestamps[it]))

    random.shuffle(items)
    
    with multiprocessing.Pool(processes=multiprocessing.cpu_count()) as pool:
        task_iterator = pool.imap_unordered(func, items, chunksize=2)
        for result_list in tqdm(task_iterator, total=len(items)):
            metrics.add_many(result_list)