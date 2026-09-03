#!/usr/bin/env python3
"""Median and worst per (scenario, metric) from results.tsv."""
import sys, collections, statistics, re
path = sys.argv[1]
data = collections.OrderedDict()
for line in open(path):
    parts = line.rstrip("\n").split("\t")
    if len(parts) != 4:
        continue
    sc, it, metric, value = parts
    data.setdefault((sc, metric), []).append((it, value))

def num(v):
    m = re.match(r"^\+?(?:(\d+)s)?(\d+)ms$", v.strip())
    if m:
        return (int(m.group(1) or 0)) * 1000 + int(m.group(2))
    m = re.match(r"^(\d+)\s*\((\d+(?:\.\d+)?)%\)$", v.strip())
    if m:
        return float(m.group(2))
    m = re.match(r"^(\d+(?:\.\d+)?)\s*ms$", v.strip())
    if m:
        return float(m.group(1))
    try:
        return float(v)
    except ValueError:
        return None

for (sc, metric), items in data.items():
    vals = [num(v) for _, v in items]
    if all(x is not None for x in vals) and vals:
        med = statistics.median(vals)
        worst = max(vals)
        raw = ", ".join(f"{v}" for _, v in items)
        print(f"{sc}\t{metric}\tmedian={med:g}\tworst={worst:g}\truns=[{raw}]")
    else:
        raw = ", ".join(f"{v}" for _, v in items)
        print(f"{sc}\t{metric}\t-\t-\truns=[{raw}]")
