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

    def save(self, folder: str):
        date = datetime.now().isoformat()
        path = f"{folder}/{self.name}-{date}.csv"
        os.makedirs(folder, exist_ok=True)
        with open(path, "w") as f:
            f.write("for_project,phase,date,value\n")
            for value in self.values:
                f.write(f"{value.project},{value.phase},{value.date.isoformat()},{value.value}\n")

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