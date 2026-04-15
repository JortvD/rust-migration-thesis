from datetime import datetime
import os
from typing import Optional
import polars as pl
from scipy.stats import linregress
import numpy as np

phases = pl.read_csv("../input/phases.csv", has_header=True)

def get_phase(project: str, date: datetime, is_rust: bool = False) -> Optional[str]:
    for row in phases.iter_rows(named=True):
        date_from = datetime.fromisoformat(row["from"]) if row["from"] != "-inf" else datetime.fromisoformat("1970-01-01T00:00:00Z")
        date_to = datetime.fromisoformat(row["to"]) if row["to"] != "inf" else datetime.fromisoformat("9999-12-31T23:59:59Z")
        
        # print(f"{row["as_project"]} == {project} && {date_from} ({date_from.tzinfo}) <= {date} ({date.tzinfo}) < {date_to} ({date_to.tzinfo})")
        if row["as_project"] == project and date_from <= date < date_to:
            return row["type"] + ("_rust" if is_rust else "")
    
    return None

def get_for_project(project: str) -> Optional[str]:
    for row in phases.iter_rows(named=True):
        if row["as_project"] == project:
            return row["for_project"]
    
    return None

class Metric:
    name: str

    def __init__(self, name: str):
        self.name = name

    def save(self, folder: str):
        pass

class SumValue:
    project: str
    phase: str
    value: float

    def __init__(self, project: str, phase: str, value: float):
        self.project = project
        self.phase = phase
        self.value = value

class SumMetric(Metric):
    value: list[SumValue]
    rust_split: bool

    def __init__(self, name: str, rust_split: bool = False):
        super().__init__(name)
        self.value = []
        self.rust_split = rust_split

    def add(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        project = get_for_project(as_project)
        phase = get_phase(as_project, date, self.rust_split and is_rust)
        
        existing_value = next((v for v in self.value if v.project == project and v.phase == phase), None)
        if existing_value:
            existing_value.value += value
        else:
            self.value.append(SumValue(project=project, phase=phase, value=value))

    def add_many(self, items: list[tuple[str, datetime, float, bool]]):
        for project, date, value, is_rust in items:
            self.add(project, date, value, is_rust)

    def save(self, folder: str):
        date = datetime.now().isoformat()
        path = f"{folder}/all.csv"
        os.makedirs(folder, exist_ok=True)
        with open(path, "a") as f:
            for value in self.value:
                f.write(f"{value.project},{self.name},{date},{value.phase},{value.value}\n")

class RatioValue:
    project: str
    phase: str
    num: float
    den: float

    def __init__(self, project: str, phase: str, num: float, den: float):
        self.project = project
        self.phase = phase
        self.num = num
        self.den = den

class RatioMetric(Metric):
    value: list[RatioValue]
    rust_split: bool

    def __init__(self, name: str, rust_split: bool = False):
        super().__init__(name)
        self.value = []
        self.rust_split = rust_split

    def add_num(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        project = get_for_project(as_project)
        phase = get_phase(as_project, date, self.rust_split and is_rust)
        
        existing_value = next((v for v in self.value if v.project == project and v.phase == phase), None)
        if existing_value:
            existing_value.num += value
        else:
            self.value.append(RatioValue(project=project, phase=phase, num=value, den=0))

    def add_den(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        project = get_for_project(as_project)
        phase = get_phase(as_project, date, self.rust_split and is_rust)

        existing_value = next((v for v in self.value if v.project == project and v.phase == phase), None)
        if existing_value:
            existing_value.den += value
        else:
            self.value.append(RatioValue(project=project, phase=phase, num=0, den=value))

    def save(self, folder: str):
        date = datetime.now().isoformat()
        path = f"{folder}/all.csv"
        os.makedirs(folder, exist_ok=True)
        with open(path, "a") as f:
            for value in self.value:
                f.write(f"{value.project},{self.name},{date},{value.phase},{value.num/value.den if value.den != 0 else 0}\n")
                f.write(f"{value.project},{self.name}_num,{date},{value.phase},{value.num}\n")
                f.write(f"{value.project},{self.name}_den,{date},{value.phase},{value.den}\n")

class TimestampValue:
    project: str
    phase: str
    date: datetime
    value: float

    def __init__(self, project: str, phase: str, date: datetime, value: float):
        self.project = project
        self.phase = phase
        self.date = date
        self.value = value

class TimestampMetric(Metric):
    values: list[TimestampValue]
    rust_split: bool

    def __init__(self, name: str, rust_split: bool = False):
        super().__init__(name)
        self.values = []
        self.rust_split = rust_split
    
    def add(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        if value is None:
            return

        project = get_for_project(as_project)
        if project is None:
            return
        
        phase = get_phase(as_project, date, self.rust_split and is_rust)
        if phase is None:
            return

        self.values.append(TimestampValue(project=project, phase=phase, date=date, value=value))

    def add_many(self, items: list[tuple[str, datetime, float, bool]]):
        for project, date, value, is_rust in items:
            self.add(project, date, value, is_rust)

    def save_phase_result(self, folder: str, name: str, func, min_items: int = 0):
        date = datetime.now().isoformat()
        path = f"{folder}/all.csv"
        os.makedirs(folder, exist_ok=True)
        with open(path, "a") as f:
            phases = set(v.phase for v in self.values)
            for project in set(v.project for v in self.values):
                for phase in phases:
                    items = [v for v in self.values if v.phase == phase and v.project == project and v.value is not None]
                    if len(items) < min_items:
                        continue
                    result = func(items) if items else None
                    f.write(f"{project},{name},{date},{phase},{result}\n")

    def save_phase_trend_slope(self, folder: str):
        self.save_phase_result(
            folder, 
            f"{self.name}_trend_slope", 
            lambda items: linregress([v.date.timestamp() / (3600 * 24 * 30) for v in items], [v.value for v in items]).slope,
            min_items=2
        )
    
    def save_phase_trend_pvalue(self, folder: str):
        if min([v.value for v in self.values if v.value is not None]) == max([v.value for v in self.values if v.value is not None]):
            print(f"Warning: All values for {self.name} are the same, skipping trend slope calculation")
            return
        self.save_phase_result(
            folder, 
            f"{self.name}_trend_pvalue", 
            lambda items: linregress([v.date.timestamp() / (3600 * 24 * 30) for v in items], [v.value for v in items]).pvalue,
            min_items=2
        )

    def save_phase_trend_rvalue(self, folder: str):
        if min([v.value for v in self.values if v.value is not None]) == max([v.value for v in self.values if v.value is not None]):
            print(f"Warning: All values for {self.name} are the same, skipping trend slope calculation")
            return
        self.save_phase_result(
            folder, 
            f"{self.name}_trend_rvalue", 
            lambda items: linregress([v.date.timestamp() / (3600 * 24 * 30) for v in items], [v.value for v in items]).rvalue,
            min_items=2
        )

    def save_phase_mean(self, folder: str):
        self.save_phase_result(
            folder, 
            f"{self.name}_mean", 
            lambda items: np.mean([v.value for v in items]),
            min_items=1
        )

    def save_phase_median(self, folder: str):
        self.save_phase_result(
            folder, 
            f"{self.name}_median", 
            lambda items: np.median([v.value for v in items]),
            min_items=1
        )

    def save_phase_emwa(self, folder: str, span: int = 5):
        self.save_phase_result(
            folder, 
            f"{self.name}_emwa", 
            lambda items: pl.Series(
                [v.value for v in sorted(items, key=lambda x: x.date)], dtype=pl.Float64
            ).ewm_mean(span=span, adjust=False).tail(1).item() if items else None,
            min_items=1
        )

    def save_phase_stdev(self, folder: str):
        self.save_phase_result(
            folder, 
            f"{self.name}_stdev", 
            lambda items: np.std([v.value for v in items]),
            min_items=1
        )

    def save(self, folder: str):
        date = datetime.now().isoformat()
        path = f"{folder}/{self.name}-{date}.csv"
        os.makedirs(folder, exist_ok=True)
        with open(path, "w") as f:
            f.write("for_project,phase,date,value\n")
            for value in self.values:
                f.write(f"{value.project},{value.phase},{value.date.isoformat()},{value.value}\n")
        # self.save_phase_trend_slope(folder)
        # self.save_phase_trend_pvalue(folder)
        # self.save_phase_trend_rvalue(folder)
        # self.save_phase_mean(folder)
        # self.save_phase_median(folder)
        # self.save_phase_emwa(folder)
        # self.save_phase_stdev(folder)

class MonthRatioValue:
    project: str
    phase: str
    month: datetime
    num: float
    den: float

    def __init__(self, project: str, phase: str, month: datetime, num: float, den: float):
        self.project = project
        self.phase = phase
        self.month = month
        self.num = num
        self.den = den

    @property
    def value(self) -> float:
        return self.num / self.den if self.den != 0 else 0
    
    @property
    def date(self) -> datetime:
        return self.month

class MonthRatioMetric(TimestampMetric):
    values: list[MonthRatioValue]
    rust_split: bool

    def __init__(self, name: str, rust_split: bool = False):
        super().__init__(name)
        self.values = []
        self.rust_split = rust_split

    def add(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        raise NotImplementedError("Use add_num and set_den instead")

    def add_num(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        project = get_for_project(as_project)
        phase = get_phase(as_project, date, self.rust_split and is_rust)
        month = datetime(date.year, date.month, 1)

        existing_value = next((v for v in self.values if v.project == project and v.phase == phase and v.month == month), None)
        if existing_value:
            existing_value.num += value
        else:
            self.values.append(MonthRatioValue(project=project, phase=phase, month=month, num=value, den=0))

    def set_den(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        project = get_for_project(as_project)
        phase = get_phase(as_project, date, self.rust_split and is_rust)
        month = datetime(date.year, date.month, 1)

        existing_value = next((v for v in self.values if v.project == project and v.phase == phase and v.month == month), None)
        if existing_value:
            existing_value.den = value
        else:
            self.values.append(MonthRatioValue(project=project, phase=phase, month=month, num=0, den=value))

    def save(self, folder: str):
        date = datetime.now().isoformat()
        path = f"{folder}/{self.name}-{date}.csv"
        os.makedirs(folder, exist_ok=True)
        with open(path, "w") as f:
            f.write("for_project,phase,date,value,num,den\n")
            for value in self.values:
                f.write(f"{value.project},{value.phase},{value.month.isoformat()},{value.num/value.den if value.den != 0 else 0},{value.num},{value.den}\n")
        self.save_phase_trend_slope(folder)
        self.save_phase_trend_pvalue(folder)
        self.save_phase_trend_rvalue(folder)
        self.save_phase_mean(folder)
        self.save_phase_median(folder)
        self.save_phase_emwa(folder)
        self.save_phase_stdev(folder)

class MonthValue:
    project: str
    phase: str
    month: datetime
    value: float

    def __init__(self, project: str, phase: str, month: datetime, value: float):
        self.project = project
        self.phase = phase
        self.month = month
        self.value = value

    @property
    def date(self) -> datetime:
        return self.month

class MonthMetric(TimestampMetric):
    values: list[MonthValue]
    rust_split: bool

    def __init__(self, name: str, rust_split: bool = False):
        super().__init__(name)
        self.values = []
        self.rust_split = rust_split

    def add(self, as_project: str, date: datetime, value: float, is_rust: bool = False):
        project = get_for_project(as_project)
        phase = get_phase(as_project, date, self.rust_split and is_rust)
        month = datetime(date.year, date.month, 1)

        existing_value = next((v for v in self.values if v.project == project and v.phase == phase and v.month == month), None)
        if existing_value:
            existing_value.value += value
        else:
            self.values.append(MonthValue(project=project, phase=phase, month=month, value=value))

    def add_many(self, items: list[tuple[str, datetime, float, bool]]):
        for project, date, value, is_rust in items:
            self.add(project, date, value, is_rust)

    def save(self, folder: str):
        date = datetime.now().isoformat()
        path = f"{folder}/{self.name}-{date}.csv"
        os.makedirs(folder, exist_ok=True)
        with open(path, "w") as f:
            f.write("for_project,phase,date,value\n")
            for value in self.values:
                f.write(f"{value.project},{value.phase},{value.month.isoformat()},{value.value}\n")
        self.save_phase_trend_slope(folder)
        self.save_phase_trend_pvalue(folder)
        self.save_phase_trend_rvalue(folder)
        self.save_phase_mean(folder)
        self.save_phase_median(folder)
        self.save_phase_emwa(folder)
        self.save_phase_stdev(folder)