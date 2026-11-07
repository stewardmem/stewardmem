#!/usr/bin/env python3
"""Word Error Rate for the OCR fixture corpus (IMPROVE05 S4).

WER = the edit distance (substitutions + deletions + insertions) between the
REFERENCE word sequence and the HYPOTHESIS word sequence, divided by the
reference length. The method is the deterministic levenshtein over word
tokens — the standard OCR fidelity metric, computed here with zero
dependencies so the measurement is re-runnable by reading this file.

Tokens are UNICODE word runs (Python's \\w+, per file order) lowercased after
casefold; punctuation is not a token (it is not the OCR fidelity question —
faithful transcription of the WORDS is, arXiv:2611.04874). Both files are
read as UTF-8 with a `\ufeff` BOM strip.

    python3 -I scripts/wer.py reference.txt hypothesis.txt
        → `WER 12.34% (S=3 D=1 I=2 ref=48 hyp=46)` and exit 0
    python3 -I scripts/wer.py --json reference.txt hypothesis.txt
        → the machine-readable record the evidence table cites

Exit 1 on a usage/read error; a WER of 0–100+ is a normal result (I>0 makes
WER exceed 1.0; the number is reported, never clamped — honest arithmetic).
"""

import json
import re
import sys

_TOKEN = re.compile(r"\w+", re.UNICODE)


def tokens(text: str) -> list[str]:
    """The word tokens: unicode word runs, casefolded, punctuation dropped."""
    text = text.lstrip("\ufeff")
    return [t.casefold() for t in _TOKEN.findall(text)]


def levenshtein_ops(ref: list[str], hyp: list[str]) -> tuple[int, int, int, int]:
    """The DP table's minimal edit script, backtracked into S/D/I counts.

    Returns (distance, substitutions, deletions, insertions). Ties in the
    backtrack prefer substitution > deletion > insertion (fixed order, so
    the decomposition is deterministic for a given pair).
    """
    n, m = len(ref), len(hyp)
    # dp[i][j] = distance between ref[:i] and hyp[:j]
    dp = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n + 1):
        dp[i][0] = i
    for j in range(m + 1):
        dp[0][j] = j
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            cost = 0 if ref[i - 1] == hyp[j - 1] else 1
            dp[i][j] = min(
                dp[i - 1][j] + 1,  # deletion
                dp[i][j - 1] + 1,  # insertion
                dp[i - 1][j - 1] + cost,  # substitution / match
            )
    # backtrack
    s = d = ins = 0
    i, j = n, m
    while i > 0 or j > 0:
        if i > 0 and j > 0 and dp[i][j] == dp[i - 1][j - 1] + (
            0 if ref[i - 1] == hyp[j - 1] else 1
        ):
            if ref[i - 1] != hyp[j - 1]:
                s += 1
            i, j = i - 1, j - 1
        elif i > 0 and dp[i][j] == dp[i - 1][j] + 1:
            d += 1
            i -= 1
        else:
            ins += 1
            j -= 1
    return dp[n][m], s, d, ins


def wer(ref_text: str, hyp_text: str) -> dict:
    ref, hyp = tokens(ref_text), tokens(hyp_text)
    if not ref:
        return {
            "wer": None,
            "substitutions": 0,
            "deletions": 0,
            "insertions": 0,
            "reference_words": 0,
            "hypothesis_words": len(hyp),
            "note": "empty reference",
        }
    dist, s, d, i = levenshtein_ops(ref, hyp)
    return {
        "wer": dist / len(ref),
        "substitutions": s,
        "deletions": d,
        "insertions": i,
        "reference_words": len(ref),
        "hypothesis_words": len(hyp),
    }


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if a != "--json"]
    as_json = "--json" in argv[1:]
    if len(args) != 2:
        print(__doc__.strip(), file=sys.stderr)
        return 1
    try:
        with open(args[0], encoding="utf-8") as f:
            ref = f.read()
        with open(args[1], encoding="utf-8") as f:
            hyp = f.read()
    except OSError as e:
        print(f"wer: {e}", file=sys.stderr)
        return 1
    result = wer(ref, hyp)
    if as_json:
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    else:
        if result["wer"] is None:
            print("WER n/a (empty reference)")
            return 0
        pct = result["wer"] * 100
        print(
            f"WER {pct:.2f}% (S={result['substitutions']}"
            f" D={result['deletions']} I={result['insertions']}"
            f" ref={result['reference_words']} hyp={result['hypothesis_words']})"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
