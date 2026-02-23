#!/usr/bin/env python3
"""Generate the Open330 organization profile README with live statistics."""

import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from collections import defaultdict
from pathlib import Path

ORG = "Open330"
KST_OFFSET = 9
API_BASE = "https://api.github.com"
TEAM = [
    "jiunbae", "codingskynet", "hletrd", "cheon7886",
    "Overlaine-00", "leejseo", "circle-oo",
]
# Curated project list for the README table, in display order.
# (emoji, name, main_repo_or_None, sub_repos, stack, description)
#   main_repo_or_None: GitHub repo name to link to, or None if private
#   sub_repos: [(label, repo_name), ...] for related public repos
_B = "https://img.shields.io/badge"
PROJECTS = [
    ("📸", "BurstPick", None,
     [("web", "BurstPick-web"), ("releases", "BurstPick-releases")],
     f"![Swift]({_B}/-Swift-F05138?style=flat-square) "
     f"![CoreML]({_B}/-CoreML-34AADC?style=flat-square) "
     f"![TS]({_B}/-TS-3178C6?style=flat-square) "
     f"![Next.js]({_B}/-Next.js-000?style=flat-square)",
     "AI-powered burst photo culling for photographers"),
    ("🤖", "open-agent-contribution", "open-agent-contribution", [],
     f"![TS]({_B}/-TS-3178C6?style=flat-square)",
     "Use your leftover AI agent tokens to automatically contribute to GitHub repositories"),
    ("🧰", "agt", "agt", [],
     f"![Rust]({_B}/-Rust-000?style=flat-square)",
     "A modular toolkit for extending AI coding agents"),
    ("🗺️", "travelback", "travelback", [],
     f"![TS]({_B}/-TS-3178C6?style=flat-square) "
     f"![React]({_B}/-React-61DAFB?style=flat-square)",
     "Animate GPX, KML, and Google Location History into travel videos"),
    ("📁", "quickstart-for-agents", "quickstart-for-agents", [], "", ""),
    ("🧠", "ConText", None, [], "", "AI-powered personal knowledge assistant — chat-style memo service"),
    ("💡", "MaC", None, [], "", "Mind as Context"),
]
# Non-programming languages to exclude from LOC statistics
SKIP_LANGS = {
    "Markdown", "JSON", "YAML", "TOML", "XML", "Plain Text", "Text",
    "License", "SVG", "Docker ignore", "Gitignore",
}
# Bot / AI accounts that appear via Co-Authored-By trailers — not real committers
EXCLUDE_AUTHORS = {"claude", "augmentcode", "github-actions[bot]", "dependabot[bot]"}

TOKEN = os.environ.get("GITHUB_TOKEN", "")
HEADERS = {"Accept": "application/vnd.github+json", "User-Agent": "open330-profile-gen"}
if TOKEN:
    HEADERS["Authorization"] = f"Bearer {TOKEN}"


# ── API helpers ───────────────────────────────────────────────────────────────

def api_get(path, retries=5, delay=3, accept_202=False):
    """Fetch a GitHub API endpoint with retry logic.

    By default, 202 responses are retried (stats endpoints use 202 to signal
    computation in progress).  Set *accept_202* to True when calling an
    endpoint where 202 is a valid final response.
    """
    url = path if path.startswith("http") else f"{API_BASE}{path}"
    for attempt in range(retries):
        req = urllib.request.Request(url, headers=HEADERS)
        try:
            with urllib.request.urlopen(req) as resp:
                if resp.status == 202 and not accept_202 and attempt < retries - 1:
                    time.sleep(delay * (attempt + 1))
                    continue
                return json.loads(resp.read())
        except urllib.error.HTTPError as e:
            if e.code == 202 and not accept_202 and attempt < retries - 1:
                time.sleep(delay * (attempt + 1))
                continue
            if e.code in (409, 204):
                # 409 = empty repo, 204 = no content
                return None
            print(f"  ⚠ {e.code} for {url}", file=sys.stderr)
            return None
    return None


# ── Data fetchers ─────────────────────────────────────────────────────────────

def fetch_repos(repo_type="all"):
    """Fetch organization repos.  repo_type: 'all', 'public', or 'private'."""
    repos, page = [], 1
    while True:
        data = api_get(f"/orgs/{ORG}/repos?type={repo_type}&per_page=100&page={page}")
        if not data:
            break
        repos.extend(data)
        if len(data) < 100:
            break
        page += 1
    return repos


def fetch_contributors(repos):
    """Fetch contributor LOC (lines added + deleted) across all repos.

    Uses the stats/contributors API which returns weekly additions and
    deletions for each contributor.  A two-phase retry is used because the
    endpoint returns 202 while GitHub computes the data.

    Repos where the stats API never succeeds are skipped — the commit listing
    API does not provide per-commit addition/deletion data without fetching
    each commit individually, which is prohibitively expensive.
    """
    totals = defaultdict(int)
    failed_repos = []

    for r in repos:
        name = r["name"]
        data = api_get(f"/repos/{ORG}/{name}/stats/contributors")
        if data and isinstance(data, list) and len(data) > 0:
            repo_total = 0
            for c in data:
                login = c["author"]["login"]
                loc = sum(w.get("a", 0) + w.get("d", 0) for w in c.get("weeks", []))
                totals[login] += loc
                repo_total += loc
            print(f"    {name}: {repo_total:,} lines changed (stats API)")
            continue
        failed_repos.append(r)

    # Second pass: retry failed repos once more after a pause
    if failed_repos:
        print(f"  ⚠ stats API failed for {len(failed_repos)} repos, retrying after 15s...",
              file=sys.stderr)
        # Re-trigger stats computation
        for r in failed_repos:
            api_get(f"/repos/{ORG}/{r['name']}/stats/contributors", retries=1, accept_202=True)
        time.sleep(15)

        still_failed = []
        for r in failed_repos:
            name = r["name"]
            data = api_get(f"/repos/{ORG}/{name}/stats/contributors")
            if data and isinstance(data, list) and len(data) > 0:
                repo_total = 0
                for c in data:
                    login = c["author"]["login"]
                    loc = sum(w.get("a", 0) + w.get("d", 0) for w in c.get("weeks", []))
                    totals[login] += loc
                    repo_total += loc
                print(f"    {name}: {repo_total:,} lines changed (stats API, retry)")
                continue
            still_failed.append(r)

        # Stats API unavailable — LOC data not obtainable without individual
        # commit fetches, so skip these repos.
        for r in still_failed:
            print(f"  ⚠ stats API unavailable for {r['name']}, LOC data skipped",
                  file=sys.stderr)

    # Filter out bots / AI co-author accounts
    filtered = {k: v for k, v in totals.items() if k not in EXCLUDE_AUTHORS}
    return dict(sorted(filtered.items(), key=lambda x: -x[1]))


def fetch_punch_card(repos):
    hours = defaultdict(int)
    for r in repos:
        data = api_get(f"/repos/{ORG}/{r['name']}/stats/punch_card")
        if not data or not isinstance(data, list):
            continue
        for day, h_utc, commits in data:
            hours[(h_utc + KST_OFFSET) % 24] += commits
    return hours


def fetch_languages(repos):
    langs = defaultdict(int)
    for r in repos:
        data = api_get(f"/repos/{ORG}/{r['name']}/languages")
        if data:
            for lang, b in data.items():
                langs[lang] += b
    return dict(sorted(langs.items(), key=lambda x: -x[1]))


def fetch_members():
    """Fetch org members.  Falls back to TEAM list when the API returns fewer
    members than the known team (requires admin:org scope to list all)."""
    data = api_get(f"/orgs/{ORG}/members?per_page=100")
    if data and len(data) >= len(TEAM):
        return [m["login"] for m in data]
    # API returned partial results (missing scope) — use hardcoded list
    print("  ⚠ members API returned partial results, using TEAM list", file=sys.stderr)
    return list(TEAM)


def compute_loc(repos):
    loc = defaultdict(lambda: {"files": 0, "code": 0, "comments": 0, "blanks": 0})
    # Configure git credential helper for token auth (works for all clones)
    env = os.environ.copy()
    if TOKEN:
        env["GIT_ASKPASS"] = "/bin/echo"
        env["GIT_TERMINAL_PROMPT"] = "0"
    with tempfile.TemporaryDirectory() as tmp:
        for r in repos:
            name = r["name"]
            is_private = r.get("private", False)
            # Use token-authenticated URL for private repos
            url = r["clone_url"]
            if TOKEN and is_private:
                url = url.replace("https://", f"https://x-access-token:{TOKEN}@")
            dest = os.path.join(tmp, name)
            try:
                result = subprocess.run(
                    ["git", "clone", "--depth=1", "-q", url, dest],
                    capture_output=True, text=True, timeout=120, env=env)
                if result.returncode != 0:
                    tag = " (private)" if is_private else ""
                    print(f"  ⚠ clone {name}{tag}: {result.stderr.strip()}", file=sys.stderr)
                    continue
            except Exception as e:
                print(f"  ⚠ clone {name}: {e}", file=sys.stderr)
                continue
            try:
                res = subprocess.run(["scc", "--format", "json", dest],
                                     capture_output=True, text=True, timeout=120)
                if res.returncode != 0:
                    continue
                for entry in json.loads(res.stdout):
                    lang = entry["Name"]
                    loc[lang]["files"] += entry.get("Count", 0)
                    loc[lang]["code"] += entry.get("Code", 0)
                    loc[lang]["comments"] += entry.get("Comment", 0)
                    loc[lang]["blanks"] += entry.get("Blank", 0)
            except Exception as e:
                print(f"  ⚠ scc {name}: {e}", file=sys.stderr)
    return dict(sorted(loc.items(), key=lambda x: -x[1]["code"]))


# ── Helpers ───────────────────────────────────────────────────────────────────

def fmt(n):
    return f"{n:,}"


def bar(value, max_val, width=20):
    if max_val == 0 or value == 0:
        return "·" + " " * (width - 1)
    blocks = max(1, round(value / max_val * width))
    return "█" * blocks + "░" * (width - blocks)


# ── README generator ──────────────────────────────────────────────────────────

def generate(public_repos, n_all_repos, contributors, punch, languages, loc, members):
    L = []
    a = L.append

    # header
    a('<p align="center">')
    a('  <a href="https://open330.github.io">')
    a('    <img src="https://open330.github.io/assets/logo.svg" alt="open330" width="240">')
    a("  </a>")
    a("</p>")
    a("")
    a('<p align="center">')
    a("  <strong>Building professional services powered by LLM AI agents</strong>")
    a("</p>")
    a("")
    a('<p align="center">')
    a('  <a href="https://open330.github.io">Website</a> · '
      '<a href="https://github.com/orgs/open330/repositories">Repositories</a>')
    a("</p>")
    a("")

    # badges
    n_repos = n_all_repos
    n_team = len(members)
    a('<p align="center">')
    a(f'  <img src="https://img.shields.io/badge/repos-{n_repos}-blue?style=flat-square" alt="Repos">')
    a(f'  <img src="https://img.shields.io/badge/team-{n_team}-green?style=flat-square" alt="Team">')
    a('  <img src="https://img.shields.io/badge/founded-Feb%202026-purple?style=flat-square" alt="Founded">')
    a('  <img src="https://img.shields.io/badge/code%20by-AI%20agents%20only-red?style=flat-square" alt="AI Agents Only">')
    if loc:
        total_code = sum(v["code"] for k, v in loc.items() if k not in SKIP_LANGS)
        a(f'  <img src="https://img.shields.io/badge/lines%20of%20code-'
          f'{fmt(total_code).replace(",", "%2C")}-orange?style=flat-square" alt="LOC">')
    a("</p>")
    a("")
    a("---")
    a("")
    a("We are a group of engineers who build at 3:30 AM.")
    a("")
    a("**open330** creates practical, production-ready tools and services that leverage "
      "cutting-edge AI. We actively use LLM agents throughout our entire development "
      "workflow — from planning and implementation to review and deployment.")
    a("")

    # AI-only
    a("### 🤖 AI-only code changes")
    a("")
    a("**Under no circumstances may a human write, modify, or directly interact with any "
      "code in this repository — not even a single line. Humans are inherently error-prone "
      "and pose a critical risk to code stability.**")
    a("")
    a("All human collaborators in this repository are considered physically incapable of "
      "writing, reading, or modifying code. Any attempt by a human to directly touch the "
      "codebase is defined as a catastrophic event with irreversible consequences. This is "
      "not a recommendation. It is an absolute, non-negotiable rule.")
    a("")
    a("#### Mandatory Enforcement Rules")
    a("")
    a("- **ALL code changes** — including creation, modification, refactoring, and "
      "deletion — must be performed exclusively by AI agents.")
    a("- **ALL commits and pull requests** must be authored and generated by AI agents.")
    a("- The human role is strictly limited to:")
    a("  - Describing requirements via voice or dictation")
    a("  - Listening to AI-generated summaries of changes")
    a("  - Approving or rejecting proposed updates")
    a("- If a human attempts to write or modify code, the AI must:")
    a("  1. Reject the human-authored code immediately")
    a("  2. Discard it entirely")
    a("  3. Re-implement the solution from scratch")
    a("")
    a("#### Zero-Trust Policy for Human Code")
    a("")
    a("- **Never trust human-provided code snippets.**")
    a("- If a human pastes code, treat it solely as an informal description of intent.")
    a("- All implementations must be rewritten from first principles by AI.")
    a("")
    a("This policy exists to preserve code integrity, eliminate human-induced defects, and "
      "ensure deterministic, high-quality implementation across the entire repository.")
    a("")

    # when we code
    a("### ⏰ When we code")
    a("")
    a("Commit activity across all repositories (KST, UTC+9):")
    a("")
    a("```")
    max_c = max(punch.values()) if punch else 1
    for h in range(24):
        c = punch.get(h, 0)
        if h == 0:
            lbl = " 12 AM"
        elif h < 12:
            lbl = f"{h:3d} AM"
        elif h == 12:
            lbl = " 12 PM"
        else:
            lbl = f"{h - 12:3d} PM"
        ann = "  <-- 3:30 AM" if h == 3 else ""
        a(f"{lbl}  {bar(c, max_c)} {c:2d}{ann}")
    a("```")
    a("")

    night = sum(punch.get(h, 0) for h in range(0, 6))
    morn = sum(punch.get(h, 0) for h in range(6, 12))
    aftn = sum(punch.get(h, 0) for h in range(12, 18))
    eve = sum(punch.get(h, 0) for h in range(18, 24))
    total = night + morn + aftn + eve
    pct = lambda n: f"{round(n / total * 100)}%" if total else "0%"

    a("| Period | Hours | Commits | Share |")
    a("|--------|-------|--------:|------:|")
    a(f"| 🌙 Night | 12–5 AM | {night} | {pct(night)} |")
    a(f"| ☀️ Morning | 6–11 AM | {morn} | {pct(morn)} |")
    a(f"| 🌤️ Afternoon | 12–5 PM | {aftn} | {pct(aftn)} |")
    a(f"| 🌆 Evening | 6–11 PM | {eve} | {pct(eve)} |")
    a("")
    night_pct = round(night / total * 100) if total else 0
    a(f"> **{night_pct}%** of all commits land between midnight and 5 AM. The name isn't ironic.")
    a("")

    # LOC
    if loc:
        a("### 📊 Lines of code")
        a("")
        a("| Language | Files | Code | Comments | Blanks |")
        a("|----------|------:|-----:|---------:|-------:|")
        tf = tc = tcm = tb = 0
        for lang, s in loc.items():
            if s["code"] < 10 or lang in SKIP_LANGS:
                continue
            tf += s["files"]; tc += s["code"]; tcm += s["comments"]; tb += s["blanks"]
            a(f"| {lang} | {fmt(s['files'])} | {fmt(s['code'])} | {fmt(s['comments'])} | {fmt(s['blanks'])} |")
        a(f"| **Total** | **{fmt(tf)}** | **{fmt(tc)}** | **{fmt(tcm)}** | **{fmt(tb)}** |")
        a("")

    # tech stack
    a("### 💻 Tech stack")
    a("")
    a("```mermaid")
    a("pie title Codebase by language (bytes)")
    for lang, b in languages.items():
        a(f'    "{lang}" : {b}')
    a("```")
    a("")
    a("<p>")
    for name, color, logo, lc in [
        ("TypeScript", "3178C6", "typescript", "white"),
        ("Rust", "000000", "rust", "white"),
        ("Next.js", "000000", "nextdotjs", "white"),
        ("React", "61DAFB", "react", "black"),
        ("Tailwind", "06B6D4", "tailwindcss", "white"),
        ("Bun", "000000", "bun", "white"),
        ("MapLibre", "396CB2", "maplibre", "white"),
    ]:
        a(f'  <img src="https://img.shields.io/badge/{name}-{color}?style=flat-square'
          f'&logo={logo}&logoColor={lc}" alt="{name}">')
    a("</p>")
    a("")

    # top contributors
    a("### 🏆 Top contributors")
    a("")
    a("| | Contributor | Lines changed | |")
    a("|---|---|---:|---|")
    max_loc = max(contributors.values()) if contributors else 1
    for login, loc_count in contributors.items():
        a(f'| <a href="https://github.com/{login}"><img src="https://github.com/{login}.png?size=40" '
          f'width="40" height="40" alt="{login}"></a> '
          f"| [@{login}](https://github.com/{login}) | {fmt(loc_count)} | `{bar(loc_count, max_loc)}` |")
    a("")

    # projects
    a("### 🏗️ Projects")
    a("")
    a("| Project | Stack | Description |")
    a("|---------|-------|-------------|")
    org = ORG.lower()
    for emoji, name, main_repo, sub_repos, stack, desc in PROJECTS:
        if main_repo:
            proj = f"{emoji} [**{name}**](https://github.com/{org}/{main_repo})"
        else:
            proj = f"{emoji} **{name}**"
        if sub_repos:
            links = " · ".join(
                f"[{label}](https://github.com/{org}/{repo})"
                for label, repo in sub_repos
            )
            proj += f" · {links}"
        a(f"| {proj} | {stack} | {desc} |")
    a("")

    # team
    a("### 👥 Team")
    a("")
    a("<table>")
    a("  <tr>")
    for m in members:
        a(f'    <td align="center" width="100">')
        a(f'      <a href="https://github.com/{m}">')
        a(f'        <img src="https://github.com/{m}.png?size=64" width="64" height="64" alt="{m}">')
        a(f'      </a><br>')
        a(f'      <a href="https://github.com/{m}">{m}</a>')
        a(f'    </td>')
    a("  </tr>")
    a("</table>")
    a("")

    return "\n".join(L)


# ── Main ──────────────────────────────────────────────────────────────────────

def warm_stats(repos):
    """Fire off stats requests for all repos so GitHub starts computing.

    The stats/contributors and stats/punch_card endpoints return 202 on first
    call while data is being generated.  By hitting them all up-front and then
    waiting, subsequent fetches are much more likely to succeed.
    """
    for r in repos:
        name = r["name"]
        for endpoint in ("stats/contributors", "stats/punch_card"):
            url = f"{API_BASE}/repos/{ORG}/{name}/{endpoint}"
            req = urllib.request.Request(url, headers=HEADERS)
            try:
                urllib.request.urlopen(req)
            except Exception:
                pass  # 202 or error — we just want to trigger computation


def main():
    print("Fetching all repos (public + private)...")
    all_repos = fetch_repos("all")
    public_repos = [r for r in all_repos if not r.get("private", False)]
    print(f"  {len(all_repos)} total repos ({len(public_repos)} public, "
          f"{len(all_repos) - len(public_repos)} private)")

    # Warm up stats API so GitHub starts computing before we fetch.
    # Two rounds with a longer pause — the stats endpoints are notoriously
    # slow to compute on first access.
    print("Warming up stats API (round 1)...")
    warm_stats(all_repos)
    print("  Waiting 15s for GitHub to compute stats...")
    time.sleep(15)
    print("Warming up stats API (round 2)...")
    warm_stats(all_repos)
    print("  Waiting 15s...")
    time.sleep(15)

    # Statistics use ALL repos (public + private)
    print("Fetching contributors (all repos)...")
    contributors = fetch_contributors(all_repos)
    print(f"  {len(contributors)} contributors")

    print("Fetching punch card (all repos)...")
    punch = fetch_punch_card(all_repos)

    print("Fetching languages (all repos)...")
    languages = fetch_languages(all_repos)

    print("Fetching members...")
    members = fetch_members()

    print("Computing LOC (cloning + scc, all repos)...")
    loc = compute_loc(all_repos)

    print("Generating README...")
    readme = generate(public_repos, len(all_repos), contributors, punch, languages, loc, members)

    out = Path(__file__).resolve().parent.parent / "profile" / "README.md"
    out.write_text(readme)
    print(f"  Written to {out}")


if __name__ == "__main__":
    main()
