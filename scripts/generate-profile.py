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
EXTRA_PROJECTS = [
    ("📸", "BurstPick", False,
     "![Swift](https://img.shields.io/badge/-Swift-F05138?style=flat-square) "
     "![CoreML](https://img.shields.io/badge/-CoreML-34AADC?style=flat-square)",
     "AI-powered burst photo culling for photographers"),
    ("🧠", "ConText", False, "", "AI-powered personal knowledge assistant — chat-style memo service"),
    ("💡", "MaC", False, "", "Mind as Context"),
]
REPO_META = {
    "open-agent-contribution": ("🤖", "![TS](https://img.shields.io/badge/-TS-3178C6?style=flat-square)"),
    "travelback": ("🗺️", "![TS](https://img.shields.io/badge/-TS-3178C6?style=flat-square) "
                   "![React](https://img.shields.io/badge/-React-61DAFB?style=flat-square)"),
    "BurstPick-web": ("🌐", "![TS](https://img.shields.io/badge/-TS-3178C6?style=flat-square) "
                      "![Next.js](https://img.shields.io/badge/-Next.js-000?style=flat-square)"),
    "BurstPick-releases": ("📦", ""),
    "agt": ("🧰", "![Rust](https://img.shields.io/badge/-Rust-000?style=flat-square)"),
}
SKIP_REPOS = {".github", "open330.github.io"}

TOKEN = os.environ.get("GITHUB_TOKEN", "")
HEADERS = {"Accept": "application/vnd.github+json", "User-Agent": "open330-profile-gen"}
if TOKEN:
    HEADERS["Authorization"] = f"Bearer {TOKEN}"


# ── API helpers ───────────────────────────────────────────────────────────────

def api_get(path, retries=3, delay=2):
    url = path if path.startswith("http") else f"{API_BASE}{path}"
    for attempt in range(retries):
        req = urllib.request.Request(url, headers=HEADERS)
        try:
            with urllib.request.urlopen(req) as resp:
                if resp.status == 202 and attempt < retries - 1:
                    time.sleep(delay * (attempt + 1))
                    continue
                return json.loads(resp.read())
        except urllib.error.HTTPError as e:
            if e.code == 202 and attempt < retries - 1:
                time.sleep(delay * (attempt + 1))
                continue
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
    totals = defaultdict(int)
    for r in repos:
        data = api_get(f"/repos/{ORG}/{r['name']}/stats/contributors")
        if not data or not isinstance(data, list):
            continue
        for c in data:
            totals[c["author"]["login"]] += c["total"]
    return dict(sorted(totals.items(), key=lambda x: -x[1]))


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
    data = api_get(f"/orgs/{ORG}/members?per_page=100")
    return [m["login"] for m in data] if data else TEAM


def compute_loc(repos):
    loc = defaultdict(lambda: {"files": 0, "code": 0, "comments": 0, "blanks": 0})
    with tempfile.TemporaryDirectory() as tmp:
        for r in repos:
            name = r["name"]
            # Use token-authenticated URL for private repos
            url = r["clone_url"]
            if TOKEN and r.get("private"):
                url = url.replace("https://", f"https://x-access-token:{TOKEN}@")
            dest = os.path.join(tmp, name)
            try:
                subprocess.run(["git", "clone", "--depth=1", "-q", url, dest],
                               capture_output=True, timeout=120)
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
        total_code = sum(v["code"] for v in loc.values())
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
    a("> **Human must NEVER write, modify, or touch any code. Not even a single line.**")
    a("")
    a("All code in our repositories is written exclusively by AI agents. Humans describe "
      "requirements, review outputs, and approve changes — but never touch the code "
      "directly. If a human pastes code, it is treated as a rough intent description and "
      "rewritten from scratch by an agent.")
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

    deep = sum(punch.get(h, 0) for h in range(0, 6))
    dawn = sum(punch.get(h, 0) for h in range(6, 9))
    morn = sum(punch.get(h, 0) for h in range(9, 13))
    aftn = sum(punch.get(h, 0) for h in range(13, 19))
    eve = sum(punch.get(h, 0) for h in range(19, 24))
    total = deep + dawn + morn + aftn + eve
    pct = lambda n: f"{round(n / total * 100)}%" if total else "0%"

    a("| Period | Hours | Commits | Share |")
    a("|--------|-------|--------:|------:|")
    a(f"| 🌙 Deep night | 12–5 AM | {deep} | {pct(deep)} |")
    a(f"| 🌅 Dawn | 6–8 AM | {dawn} | {pct(dawn)} |")
    a(f"| ☀️ Morning | 9 AM–12 PM | {morn} | {pct(morn)} |")
    a(f"| 🌤️ Afternoon | 1–6 PM | {aftn} | {pct(aftn)} |")
    a(f"| 🌆 Evening | 7–11 PM | {eve} | {pct(eve)} |")
    a("")
    night_pct = round((deep + dawn) / total * 100) if total else 0
    a(f"> **{night_pct}%** of all commits land between midnight and 8 AM. The name isn't ironic.")
    a("")

    # LOC
    if loc:
        a("### 📊 Lines of code")
        a("")
        a("| Language | Files | Code | Comments | Blanks |")
        a("|----------|------:|-----:|---------:|-------:|")
        tf = tc = tcm = tb = 0
        for lang, s in loc.items():
            if s["code"] < 10:
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
    a("| | Contributor | Commits | |")
    a("|---|---|---:|---|")
    max_cm = max(contributors.values()) if contributors else 1
    for login, commits in contributors.items():
        a(f"| <img src=\"https://github.com/{login}.png?size=40\" width=\"40\" alt=\"{login}\"> "
          f"| [@{login}](https://github.com/{login}) | {commits} | `{bar(commits, max_cm)}` |")
    a("")

    # projects
    a("### 🏗️ Projects")
    a("")
    a("| Project | Stack | Description |")
    a("|---------|-------|-------------|")
    for r in public_repos:
        name = r["name"]
        if name in SKIP_REPOS:
            continue
        emoji, stack = REPO_META.get(name, ("📁", ""))
        desc = r.get("description") or ""
        a(f"| {emoji} [**{name}**](https://github.com/{ORG.lower()}/{name}) | {stack} | {desc} |")
    for emoji, name, has_link, stack, desc in EXTRA_PROJECTS:
        a(f"| {emoji} **{name}** | {stack} | {desc} |")
    a("")

    # team
    a("### 👥 Team")
    a("")
    avatars = " | ".join(
        f'<a href="https://github.com/{m}"><img src="https://github.com/{m}.png?size=60" '
        f'width="60" alt="{m}"></a>' for m in members)
    a(f"| {avatars} |")
    a("|" + "|".join(":---:" for _ in members) + "|")
    names = " | ".join(f"[{m}](https://github.com/{m})" for m in members)
    a(f"| {names} |")
    a("")

    return "\n".join(L)


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    print("Fetching all repos (public + private)...")
    all_repos = fetch_repos("all")
    public_repos = [r for r in all_repos if not r.get("private", False)]
    print(f"  {len(all_repos)} total repos ({len(public_repos)} public, "
          f"{len(all_repos) - len(public_repos)} private)")

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
