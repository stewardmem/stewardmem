#!/usr/bin/env bash
# Release-notes gate: everything release.yml refuses to publish for, runnable
# LOCALLY in under a second.
#
# Usage: scripts/release-notes-check.sh [version]
#   version defaults to the `version` in Cargo.toml (the release being cut).
#
# Why this exists: every one of these four checks lived ONLY inside
# `.github/workflows/release.yml`, so the ONLY place they could ever fail was
# a full public matrix plus four cross-builds — 30-45 minutes of runners to
# learn from a `grep` that costs nothing. That is not hypothetical: the
# v1.29.13 release was refused twice at exactly this step, for two different
# reasons (an empty bullet block because the categories were h3 headings
# rather than bold headings, and audit IDs `K4-05`/`K4-12` in the intro
# prose). Both were visible locally in seconds.
#
# ONE implementation, two callers: release.yml runs this as its first
# notes-step action, and `tests/release_notes_gate.rs` runs it as a test. A
# check that exists in only one of those places is a check that fails only
# when it is most expensive — which is the defect class this file is closing.
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}"
fail() { echo "::error::$1" >&2; exit 1; }

if ! grep -q "^## \[$VERSION\]" CHANGELOG.md; then
  fail "CHANGELOG.md has no '## [$VERSION]' section. release.yml generates the published notes from it and refuses to publish without one."
fi

# 1. The `### Release notes` block is mandatory.
if ! awk -v ver="$VERSION" '
      /^## \[/ { if (insec) exit; insec = ($0 ~ "^## \\[" ver "\\]"); next }
      insec && /^### Release notes/ { found = 1; exit }
      END { exit found ? 0 : 1 }
    ' CHANGELOG.md; then
  fail "CHANGELOG.md section '## [$VERSION]' is missing a '### Release notes' block — refusing to publish a release without user-facing notes."
fi

# 2. Intro prose: everything between the heading and the first subsection is
#    the published body's opening paragraph.
awk -v ver="$VERSION" '
  /^## \[/ { insec = ($0 ~ "^## \\[" ver "\\]"); next }
  !insec { next }
  /^### / { exit }
  /^\*\*M[0-9]/ { exit }
  { print }
' CHANGELOG.md > /tmp/release-notes-intro.$$
if ! grep -q '[^[:space:]]' /tmp/release-notes-intro.$$; then
  rm -f /tmp/release-notes-intro.$$
  fail "CHANGELOG.md section '## [$VERSION]' carries no prose summary between the heading and '### Release notes'. The published body opens with that summary."
fi

# 3. ONLY categorized content publishes, in either shipped shape: a bullet
#    leading with its own category, or bullets grouped under a bold category
#    heading. Indented continuation lines travel WITH their bullet. The
#    `notes && /^### / { exit }` is the subtlety that bit v1.29.13: an h3
#    category heading TERMINATES the block, so `### Security fixes` silently
#    published zero bullets.
awk -v ver="$VERSION" '
  /^## \[/ { insec = ($0 ~ "^## \\[" ver "\\]"); next }
  !insec { next }
  /^### Release notes/ { notes = 1; next }
  notes && /^### / { exit }
  notes && /^\*\*(Improvements|New|Changed|Bug fixes|Security fixes)\*\*/ {
    if (printed) print ""
    print; cat = 1; printed = 1; next
  }
  notes && /^- \*\*(Improvements|New|Changed|Bug fixes|Security fixes):/ {
    print; cat = 1; printed = 1; next
  }
  notes && cat && /^[ \t]+[^ \t]/ { print; printed = 1; next }
  notes && NF && !/^- / && !/^ / { cat = 0 }
  notes && /^- / { if (cat) print; else print "UNCATEGORIZED " $0 }
' CHANGELOG.md > /tmp/release-notes-bullets.$$

bad=$(grep -c '^UNCATEGORIZED ' /tmp/release-notes-bullets.$$ || true)
if [ "$bad" -gt 0 ]; then
  grep '^UNCATEGORIZED ' /tmp/release-notes-bullets.$$ | sed 's/^UNCATEGORIZED /::error::  /' >&2
  rm -f /tmp/release-notes-intro.$$ /tmp/release-notes-bullets.$$
  fail "CHANGELOG.md '### Release notes' for $VERSION carries $bad uncategorized bullet(s). Put them under a **Improvements**/**Bug fixes**/**Security fixes** sub-heading (bold, NOT an h3) or lead each bullet with its category."
fi

# 4. Audit/finding IDs never publish — the user-facing text names behaviour,
#    the Engineering record keeps the IDs.
ids=$(grep -nE '\b([A-Z]{1,2}[0-9]+-[0-9]+|[A-Z]-[0-9]+)\b' \
      /tmp/release-notes-intro.$$ /tmp/release-notes-bullets.$$ || true)
if [ -n "$ids" ]; then
  printf '%s\n' "$ids" | sed 's/^/::error::  /' >&2
  rm -f /tmp/release-notes-intro.$$ /tmp/release-notes-bullets.$$
  fail "audit/finding IDs must not appear in the published notes — put them in the Engineering record and name proof commits in the bullets."
fi

# 5. COMPLETENESS: everything that shipped between the previous tag and this
#    one must be named in the released section. The proof-commit convention
#    is the input: a user-facing change names the commit that landed it
#    inside its bullet, and a change that deliberately ships no note is
#    named under the section's `### Engineering record` with the reason.
#    A commit named in NEITHER place is a gap — the exact class that once
#    published a release carrying two rounds while its section described
#    only one, with every shape check above green. Two directions, both
#    fail-closed:
#      - coverage: every commit whose subject is not an engineering-only
#        marker must appear in the section (as a cited proof commit or an
#        engineering-record line) — over-inclusion is cheap (one record
#        line), under-inclusion is the published gap;
#      - integrity: every commit the section cites must actually have
#        shipped in this range — a proof that proves nothing is worse than
#        no proof.
#    Skipped where git or the tags are absent (the scratch-tree drills),
#    and the skip is PRINTED, so a disarmed arm is visible, never silent.
completeness="skipped (no git repository or missing tags)"
if git rev-parse --git-dir >/dev/null 2>&1; then
  prev=$(sed -nE 's/^## \[([0-9][0-9.]*)\].*/\1/p' CHANGELOG.md | sort -V \
        | awk -v v="$VERSION" '$0 == v { print p } { p = $0 }')
  if [ -n "$prev" ] \
     && git rev-parse -q --verify "v$prev^{commit}" >/dev/null \
     && git rev-parse -q --verify "v$VERSION^{commit}" >/dev/null; then
    awk -v ver="$VERSION" '
      /^## \[/ { insec = ($0 ~ "^## \\[" ver "\\]"); next }
      !insec { next }
      { print }
    ' CHANGELOG.md > /tmp/release-notes-section.$$
    grep -oE '[0-9a-f]{7,40}' /tmp/release-notes-section.$$ | sort -u > /tmp/release-notes-tokens.$$ || true
    git log --format='%H %s' --no-merges "v$prev..v$VERSION" > /tmp/release-notes-range.$$
    # the subject sits AFTER the sha on each line — the exclusion anchors
    # must name that gap, or they exclude nothing and every range commit
    # becomes a candidate (the first live run found exactly that).
    grep -vE '^[0-9a-f]{7,40} (chore|ci|deps|docs|doc|test|tests|style|refactor|fmt|lint|badge|release|build|Merge|merge|revert|Revert|CHANGELOG|comments?|pins|register|evidence|plans|repo|AGENTS|fuzz)([:(]|\b)' /tmp/release-notes-range.$$ \
      | grep -vE '^[0-9a-f]{7,40} (fix|feat)\((ci|docs|doc|test|tests|lint|fmt|changelog|deps|sbom|badge)\)' \
      > /tmp/release-notes-candidates.$$ || true
    missing=0
    while read -r sha subj; do
      [ -n "$sha" ] || continue
      named=0
      while read -r tok; do
        case "$sha" in "$tok"*) named=1; break;; esac
      done < /tmp/release-notes-tokens.$$
      if [ "$named" = 0 ]; then
        echo "COMPLETENESS::  ${sha:0:8} $subj" >&2
        missing=$((missing + 1))
      fi
    done < /tmp/release-notes-candidates.$$
    if [ "$missing" -gt 0 ]; then
      rm -f /tmp/release-notes-*.$$
      fail "COMPLETENESS: $missing commit(s) between v$prev and v$VERSION are named nowhere in the released section — cite each user-facing one inside its bullet (the commit that landed it), or list it under '### Engineering record' with the reason it ships no note. A round that landed between the tags and lives only in its own record publishes exactly the incomplete page this check exists to refuse."
    fi
    badcit=0
    while read -r tok; do
      [ -n "$tok" ] || continue
      c=$(git rev-parse -q --verify "$tok^{commit}" 2>/dev/null) || continue
      if git merge-base --is-ancestor "$c" "v$VERSION" 2>/dev/null \
         && ! git merge-base --is-ancestor "$c" "v$prev" 2>/dev/null; then
        continue
      fi
      echo "CITATION::  $tok does not resolve to a commit in v$prev..v$VERSION" >&2
      badcit=$((badcit + 1))
    done < /tmp/release-notes-tokens.$$
    if [ "$badcit" -gt 0 ]; then
      rm -f /tmp/release-notes-*.$$
      fail "CITATION: $badcit cited commit(s) did not ship in v$prev..v$VERSION — a proof commit must be a commit this release carries."
    fi
    cand=$(wc -l < /tmp/release-notes-candidates.$$ | tr -d ' ')
    completeness="$cand candidate commit(s) named, citations verified (range v$prev..v$VERSION)"
    rm -f /tmp/release-notes-section.$$ /tmp/release-notes-tokens.$$ /tmp/release-notes-range.$$ /tmp/release-notes-candidates.$$
  fi
fi

# 6. A category heading that ends with no bullet publishes a blank block —
#    the bold-heading twin of the h3 defect. Two shapes live in the wild:
#    the truly empty heading, and a None disclosure written as prose
#    ("None.") which the extractor cannot publish because only bullets
#    travel. Both leave a dangling heading on the page; both refuse here.
#    The honest shapes are bullets ("real bullets" or "- None.").
emptycat=$(awk '
  /^\*\*(Improvements|New|Changed|Bug fixes|Security fixes)\*\*/ {
    if (pending != "") print pending
    pending = $0
    next
  }
  NF { pending = "" }
  END { if (pending != "") print pending }
' /tmp/release-notes-bullets.$$)
if [ -n "$emptycat" ]; then
  printf '%s\n' "$emptycat" | sed 's/^/::error::  /' >&2
  rm -f /tmp/release-notes-intro.$$ /tmp/release-notes-bullets.$$
  fail "CHANGELOG.md '### Release notes' for $VERSION carries a category heading with no bullets — it publishes a blank block on the page. Write the bullets, or put the disclosure in bullet form ('- None.'); prose under a heading does not travel."
fi

bullets=$(grep -c '^- ' /tmp/release-notes-bullets.$$ || true)
if [ "$bullets" -lt 1 ]; then
  rm -f /tmp/release-notes-intro.$$ /tmp/release-notes-bullets.$$
  fail "CHANGELOG.md '### Release notes' block for $VERSION produced no bullets. A category heading written as '### X' instead of '**X**' ends the block there and publishes an empty body — write real notes, then re-tag."
fi

rm -f /tmp/release-notes-intro.$$ /tmp/release-notes-bullets.$$
echo "OK  release notes for $VERSION: intro present, $bullets categorized bullet(s), no audit IDs, completeness: $completeness"
