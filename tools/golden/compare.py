"""Compare two golden JSON files (the committed one and a freshly regenerated one) value by value.

  python3 tools/golden/compare.py committed.json regenerated.json

Numbers match within a relative tolerance of 1e-9 (different numpy builds may differ in the last bits); everything else
must be identical. Exits 1 and lists the differing paths otherwise.
"""
import json, math, sys


def walk(a, b, path, out):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a or k not in b:
                out.append(f"{path}/{k}: only in {'regenerated' if k in b else 'committed'}")
            else:
                walk(a[k], b[k], f"{path}/{k}", out)
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out.append(f"{path}: length {len(a)} vs {len(b)}")
        for i, (x, y) in enumerate(zip(a, b)):
            walk(x, y, f"{path}[{i}]", out)
    elif isinstance(a, (int, float)) and isinstance(b, (int, float)) and not isinstance(a, bool) and not isinstance(b, bool):
        if not math.isclose(a, b, rel_tol=1e-9, abs_tol=1e-12):
            out.append(f"{path}: {a} vs {b}")
    elif a != b:
        out.append(f"{path}: {a!r} vs {b!r}")


def main():
    a, b = (json.load(open(p, encoding="utf-8")) for p in sys.argv[1:3])
    out = []
    walk(a, b, "", out)
    if out:
        print(f"{len(out)} differences between {sys.argv[1]} and {sys.argv[2]}:")
        print("\n".join(out[:40]))
        sys.exit(1)
    print("golden files match")


if __name__ == "__main__":
    main()
