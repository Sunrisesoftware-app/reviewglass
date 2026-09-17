"""Summarize a ReviewGlass follow log (%TEMP%/reviewglass-follow.log).

python analyze_follow.py [path] [--from S] [--to S]

Answers the questions the conversation needs: how often the detector's reading changed
and by how much, how often the glass was told, what Fit did with it, and how often the
window actually resized — so the detector's readings can be told from Fit's reaction.
"""
import collections
import os
import re
import statistics
import sys

path = next((a for a in sys.argv[1:] if not a.startswith("--")), os.path.join(os.environ.get("TEMP", "."), "reviewglass-follow.log"))
t_from = float(sys.argv[sys.argv.index("--from") + 1]) if "--from" in sys.argv else 0.0
t_to = float(sys.argv[sys.argv.index("--to") + 1]) if "--to" in sys.argv else float("inf")

rows = []
header = []
with open(path, encoding="utf-8", errors="replace") as f:
    for line in f:
        line = line.rstrip("\n")
        if line.startswith("#"):
            header.append(line)
            continue
        parts = line.split(" ", 2)
        if len(parts) < 2:
            continue
        t = float(parts[0])
        if not (t_from <= t <= t_to):
            continue
        rows.append((t, parts[1], parts[2] if len(parts) > 2 else ""))

print("\n".join(header[:1]))
if not rows:
    print("no rows in range")
    sys.exit(0)
span = rows[-1][0] - rows[0][0]
kinds = collections.Counter(k for _, k, _ in rows)
print(f"{len(rows)} lines over {span:.1f} s: " + ", ".join(f"{k}={n}" for k, n in kinds.most_common()))


def field(s, name):
    m = re.search(rf"(?:^|\s){name}=(\S+)", s)
    return m.group(1) if m else None


# --- the detector's readings
scans = [(t, f) for t, k, f in rows if k == "scan"]
readings = []
for t, f in scans:
    p = field(f, "pane")
    if p and p != "none":
        a, rest = p.split("..")
        b, w = rest.split("/")
        readings.append((t, int(a), int(b), int(w), field(f, "changed") == "1"))
    else:
        readings.append((t, None, None, None, field(f, "changed") == "1"))
found = [r for r in readings if r[3] is not None]
print(f"\nDetector: {len(scans)} scans, {len(found)} with a column, {len(scans) - len(found)} none; "
      f"{sum(1 for r in readings if r[4])} readings counted as a change (beyond the 8 px jitter)")
if found:
    widths = [r[3] for r in found]
    print(f"  width: min {min(widths)}, median {statistics.median(widths):.0f}, max {max(widths)}; "
          f"left edge min..max {min(r[1] for r in found)}..{max(r[1] for r in found)}, "
          f"right edge min..max {min(r[2] for r in found)}..{max(r[2] for r in found)}")
    # runs of the same accepted reading: how long does a reading hold?
    hold = []
    last_t = None
    for r in readings:
        if r[4]:
            if last_t is not None:
                hold.append(r[0] - last_t)
            last_t = r[0]
    if hold:
        print(f"  a reading held for: median {statistics.median(hold):.1f} s, min {min(hold):.1f} s, max {max(hold):.1f} s "
              f"({sum(1 for h in hold if h < 2.0)} of {len(hold)} changes came within 2 s of the previous)")
    # the biggest jumps between consecutive found readings
    jumps = []
    for a, b in zip(found, found[1:]):
        jumps.append((abs(b[1] - a[1]), abs(b[2] - a[2]), a[0]))
    big = sorted(jumps, reverse=True)[:5]
    print("  largest edge jumps between consecutive scans (left px, right px, at t): " + ", ".join(f"({l},{r},{t:.1f})" for l, r, t in big))

# --- per column: is it the left edge or the right edge that moves? (left edge exact)
cols = collections.defaultdict(list)
for t, a, b, w, ch in found:
    cols[a].append(b)
if cols:
    print("  per column (grouped by the exact left edge), the right edge as read:")
    for x0, rs in sorted(cols.items(), key=lambda kv: -len(kv[1]))[:8]:
        med = statistics.median(rs)
        off = sum(1 for r in rs if abs(r - med) > 8)
        dist = ", ".join(f"{v}x{n}" for v, n in sorted(collections.Counter(rs).items()))
        print(f"    x0={x0:5d}: {len(rs):3d} readings, right edge {min(rs)}..{max(rs)} (range {max(rs) - min(rs)} px), "
              f"{off} more than 8 px off the median {med:.0f}; values {dist}")

# --- 'none' inside a column: does the source rectangle jump sideways?
srcs = [(t, int(f.split(" ")[0].split(",")[0])) for t, k, f in rows if k == "src"]
nones = [(t, f) for t, f in scans if field(f, "pane") == "none"]
if nones and srcs:
    print(f"  on each of the {len(nones)} 'none' readings, the source x before it and within the next 1.5 s:")
    for t, f in nones[:12]:
        before = [x for tt, x in srcs if tt <= t]
        after = [x for tt, x in srcs if t < tt <= t + 1.5]
        if before and after:
            b = before[-1]
            print(f"    t={t:6.1f} cur={field(f, 'cur')}: x {b} -> {'/'.join(str(x) for x in after[:5])}  (max jump {max(abs(x - b) for x in after)} px)")
    if len(nones) > 12:
        print("    …")

# --- what the glass was told
events = [(t, field(f, "width")) for t, k, f in rows if k == "pane-event"]
print(f"\nGlass told {len(events)} times" + (f" ({sum(1 for _, w in events if w == 'none')} of them 'none')" if events else ""))

# --- Fit's decisions
fits = [(t, f) for t, k, f in rows if k == "fit"]
decisions = collections.Counter()
for t, f in fits:
    m = re.search(r"-> (\S+)", f)
    decisions[m.group(1) if m else ("skip" if f.startswith("skip") else "?")] += 1
print("Fit: " + ", ".join(f"{k}={n}" for k, n in decisions.most_common()))
resizes = [(t, f) for t, k, f in rows if k == "resized"]
if resizes:
    ws = [int(f.split("x")[0]) for _, f in resizes]
    print(f"  window resized {len(resizes)} times; widths {min(ws)}..{max(ws)}; "
          f"{sum(1 for a, b in zip(ws, ws[1:]) if b > a)} wider, {sum(1 for a, b in zip(ws, ws[1:]) if b < a)} narrower")
    gaps = [b[0] - a[0] for a, b in zip(resizes, resizes[1:])]
    if gaps:
        print(f"  gap between resizes: median {statistics.median(gaps):.1f} s, min {min(gaps):.1f} s")
views = [(t, f) for t, k, f in rows if k == "view"]
derived = [f for _, f in views if field(f, "derived") == "1"]
if views:
    zooms = collections.Counter(field(f, "zoom") for _, f in views)
    print(f"  view reports {len(views)}, zoom values {dict(zooms)}, derived {len(derived)}")

# --- the source rectangle
srcs = [(t, f) for t, k, f in rows if k == "src"]
if srcs:
    xs = [int(f.split(" ")[0].split(",")[0]) for _, f in srcs]
    ws = [int(f.split(" ")[0].split(",")[2]) for _, f in srcs]
    hov = sum(1 for _, f in srcs if field(f, "hovered") == "1")
    print(f"\nSource rectangle logged {len(srcs)} times (10/s cap); x {min(xs)}..{max(xs)}, w {min(ws)}..{max(ws)}; hovered in {hov}")

# --- timeline of what happened (compressed): every change, pane-event, fit decision, resize
print("\nTimeline (changes only):")
shown = 0
for t, k, f in rows:
    if k == "scan" and field(f, "changed") != "1":
        continue
    if k in ("src", "hover"):
        continue
    print(f"  {t:8.3f} {k:10s} {f}")
    shown += 1
    if shown >= 120:
        print("  … (truncated)")
        break
