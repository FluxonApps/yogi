#!/usr/bin/env bash
# Yogi — live status reporter (akshi-terminal aesthetic). For asciinema streaming.
#   scripts/status.sh                 one-shot snapshot
#   scripts/status.sh --watch [secs]  live auto-refresh (default 5s)
#
# Reads committed truth (git, docs/MILESTONES.md) + a small live-state file
# (.yogi/status.txt, ephemeral, updated by the autonomous loop each tick).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

G=$'\033[38;5;42m'  # accent green
GB=$'\033[1;38;5;42m'
D=$'\033[38;5;240m' # dim
DD=$'\033[38;5;245m'
B=$'\033[1m'
Y=$'\033[38;5;179m'
R=$'\033[0m'
W=76

field() { grep -m1 "^$1:" .yogi/status.txt 2>/dev/null | sed "s/^$1:[[:space:]]*//"; }
rule()  { printf "${D}%s${R}\n" "$(printf '─%.0s' $(seq 1 $W))"; }
# print a label section header like ── label ───────────
sec() { local t="$1"; printf "${D}── ${DD}%s ${D}%s${R}\n" "$t" "$(printf '─%.0s' $(seq 1 $((W-5-${#t}))))"; }
# truncate to width
fit() { local s="$1" n="${2:-$W}"; if [ "${#s}" -gt "$n" ]; then printf '%s…' "${s:0:$((n-1))}"; else printf '%s' "$s"; fi; }

mstone() {
  local m="$1" line mark
  line=$(grep -m1 "^## M$m " docs/MILESTONES.md 2>/dev/null || echo "")
  if   echo "$line" | grep -q '\[x\]'; then mark="${G}M${m}✓${R}"
  elif echo "$line" | grep -q '🔒';   then mark="${Y}M${m}◌${R}"
  else mark="${D}M${m}·${R}"; fi
  printf '%s  ' "$mark"
}

render() {
  local crates commits tests build phase now why upd
  crates=$(ls crates 2>/dev/null | wc -l | tr -d ' ')
  commits=$(git rev-list --count HEAD 2>/dev/null || echo '?')
  # Live count of #[test] across all crates (src + tests/) — never goes stale (no manual field).
  tests=$(grep -rho '#\[test\]' crates --include=*.rs 2>/dev/null | wc -l | tr -d ' ')
  build=$(field BUILD); [ -z "$build" ] && build="?"
  phase=$(field PHASE); [ -z "$phase" ] && phase="autonomous loop"
  now=$(field NOW);     [ -z "$now" ]   && now="(idle)"
  why=$(field WHY);     [ -z "$why" ]   && why="—"
  upd=$(field UPDATED); [ -z "$upd" ]   && upd="(no live state yet)"

  printf '\n'
  printf "  ${GB}<0> yogi${R}   ${D}trust-native self-evolving being · local-only (qwen3:8b + bench)${R}\n\n"

  sec "now"
  printf "  ${D}phase${R}  ${B}%s${R}\n" "$(fit "$phase" $((W-9)))"
  printf "  ${D}doing${R}  %s\n" "$(fit "$now" $((W-9)))"
  printf "  ${D}why  ${R}  ${DD}%s${R}\n" "$(fit "$why" $((W-9)))"
  printf '\n'

  sec "build"
  local bcol="$G"; [ "$build" != "green" ] && bcol="$Y"
  printf "  crates ${GB}%s${R}   tests ${GB}%s${R}   commits ${GB}%s${R}   build ${bcol}● %s${R}\n\n" "$crates" "$tests" "$commits" "$build"

  sec "milestones"
  printf "  "; for m in 0 1 2 3 4 5 6; do mstone "$m"; done
  printf "  ${D}(M6 selection LIVE — recombination assembles the all-3-skill solver, 1.0)${R}\n\n"

  sec "compounding layers — model held constant"
  printf "  ${G}●${R} memory (hybrid embed+lexical)   ${D}live · wired${R}\n"
  printf "  ${G}●${R} skills (verifier-fed)           ${D}live · top-2 inject${R}\n"
  printf "  ${G}●${R} navigator / router              ${D}live · heuristic + outcome-learned${R}\n"
  printf "  ${G}●${R} distillation flywheel           ${D}live · gap→distill→gate PROMOTES${R}\n"
  printf "  ${G}●${R} population + selection (M6)      ${D}live · recombination composes skills${R}\n\n"

  sec "certified — token-space compounding (verifier-gated)"
  if grep -q '^CERT' .yogi/status.txt 2>/dev/null; then
    grep '^CERT' .yogi/status.txt | sed "s/^CERT[0-9]*:[[:space:]]*//" | while IFS= read -r l; do
      printf "  ${G}›${R} %s\n" "$(fit "$l" $((W-4)))"
    done
  else
    printf "  ${D}(awaiting live cert results)${R}\n"
  fi
  printf '\n'

  sec "recent commits"
  git log --oneline -6 2>/dev/null | while IFS= read -r l; do printf "  ${D}%s${R}\n" "$(fit "$l" $((W-2)))"; done
  printf '\n'
  printf "  ${D}updated %s · 'continue the loop' drives the next step${R}\n\n" "$upd"
}

if [ "${1:-}" = "--watch" ]; then
  secs="${2:-5}"
  while true; do printf '\033[H\033[2J'; render; sleep "$secs"; done
else
  render
fi
