use anyhow::{Context, Result};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use tempfile::TempDir;

const ORG: &str = "Open330";
const KST_OFFSET: i64 = 9;
const API_BASE: &str = "https://api.github.com";

const TEAM: &[&str] = &[
    "jiunbae",
    "codingskynet",
    "hletrd",
    "cheon7886",
    "Overlaine-00",
    "leejseo",
    "circle-oo",
];

const SKIP_LANGS: &[&str] = &[
    "Markdown",
    "JSON",
    "YAML",
    "TOML",
    "XML",
    "Plain Text",
    "Text",
    "License",
    "SVG",
    "Docker ignore",
    "Gitignore",
];

const EXCLUDE_AUTHORS: &[&str] = &[
    "claude",
    "augmentcode",
    "github-actions[bot]",
    "dependabot[bot]",
];

const BURSTPICK_SUB_REPOS: &[(&str, &str)] =
    &[("web", "BurstPick-web"), ("releases", "BurstPick-releases")];

#[derive(Clone, Copy)]
struct Project {
    emoji: &'static str,
    name: &'static str,
    main_repo: Option<&'static str>,
    sub_repos: &'static [(&'static str, &'static str)],
    stack: &'static str,
    description: &'static str,
}

const PROJECTS: &[Project] = &[
    Project {
        emoji: "📸",
        name: "BurstPick",
        main_repo: None,
        sub_repos: BURSTPICK_SUB_REPOS,
        stack: "![Swift](https://img.shields.io/badge/-Swift-F05138?style=flat-square) ![CoreML](https://img.shields.io/badge/-CoreML-34AADC?style=flat-square) ![Vision](https://img.shields.io/badge/-Vision-5AC8FA?style=flat-square) ![SwiftUI](https://img.shields.io/badge/-SwiftUI-0A84FF?style=flat-square) ![Metal](https://img.shields.io/badge/-Metal-8E8E93?style=flat-square)",
        description: "AI-powered burst photo culling for photographers",
    },
    Project {
        emoji: "🤖",
        name: "open-agent-contribution",
        main_repo: Some("open-agent-contribution"),
        sub_repos: &[],
        stack: "![TS](https://img.shields.io/badge/-TS-3178C6?style=flat-square)",
        description: "Use your leftover AI agent tokens to automatically contribute to GitHub repositories",
    },
    Project {
        emoji: "🗜️",
        name: "context-compress",
        main_repo: Some("context-compress"),
        sub_repos: &[],
        stack: "![TS](https://img.shields.io/badge/-TS-3178C6?style=flat-square) ![MCP](https://img.shields.io/badge/-MCP-4A5568?style=flat-square)",
        description: "MCP server and PreToolUse hook that compresses tool outputs to save context window",
    },
    Project {
        emoji: "🧰",
        name: "agt",
        main_repo: Some("agt"),
        sub_repos: &[],
        stack: "![Rust](https://img.shields.io/badge/-Rust-000?style=flat-square)",
        description: "A modular toolkit for extending AI coding agents",
    },
    Project {
        emoji: "🗺️",
        name: "travelback",
        main_repo: Some("travelback"),
        sub_repos: &[],
        stack: "![TS](https://img.shields.io/badge/-TS-3178C6?style=flat-square) ![React](https://img.shields.io/badge/-React-61DAFB?style=flat-square)",
        description: "Animate GPX, KML, and Google Location History into travel videos",
    },
    Project {
        emoji: "📁",
        name: "quickstart-for-agents",
        main_repo: Some("quickstart-for-agents"),
        sub_repos: &[],
        stack: "",
        description: "",
    },
    Project {
        emoji: "🧠",
        name: "ConText",
        main_repo: None,
        sub_repos: &[],
        stack: "",
        description: "AI-powered personal knowledge assistant — chat-style memo service",
    },
    Project {
        emoji: "💡",
        name: "MaC",
        main_repo: None,
        sub_repos: &[],
        stack: "",
        description: "Mind as Context",
    },
];

#[derive(Clone, Debug)]
struct Repo {
    name: String,
    private_repo: bool,
    clone_url: String,
}

#[derive(Clone, Debug, Default)]
struct LocStats {
    files: u64,
    code: u64,
    comments: u64,
    blanks: u64,
}

struct GithubClient {
    client: Client,
    token: Option<String>,
}

impl GithubClient {
    fn new(token: Option<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("open330-profile-gen"));
        if let Some(t) = &token {
            let auth = format!("Bearer {t}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth).context("failed to create auth header")?,
            );
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self { client, token })
    }

    fn api_get(
        &self,
        path: &str,
        retries: usize,
        delay_secs: u64,
        accept_202: bool,
    ) -> Option<Value> {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{API_BASE}{path}")
        };

        for attempt in 0..retries {
            let response = match self.client.get(&url).send() {
                Ok(resp) => resp,
                Err(err) => {
                    eprintln!("  ⚠ request error for {url}: {err}");
                    return None;
                }
            };

            let status = response.status();
            if status == StatusCode::ACCEPTED {
                if accept_202 {
                    return None;
                }
                if attempt < retries.saturating_sub(1) {
                    let backoff = delay_secs.saturating_mul((attempt as u64) + 1);
                    sleep(Duration::from_secs(backoff));
                    continue;
                }
                return None;
            }

            if status == StatusCode::NO_CONTENT || status == StatusCode::CONFLICT {
                return None;
            }

            if !status.is_success() {
                eprintln!("  ⚠ {} for {}", status.as_u16(), url);
                return None;
            }

            let body = match response.text() {
                Ok(text) => text,
                Err(err) => {
                    eprintln!("  ⚠ failed to read body for {url}: {err}");
                    return None;
                }
            };

            if body.trim().is_empty() {
                return None;
            }

            return match serde_json::from_str::<Value>(&body) {
                Ok(v) => Some(v),
                Err(err) => {
                    eprintln!("  ⚠ invalid JSON from {url}: {err}");
                    None
                }
            };
        }

        None
    }
}

fn skip_lang_set() -> HashSet<&'static str> {
    SKIP_LANGS.iter().copied().collect()
}

fn excluded_author_set() -> HashSet<&'static str> {
    EXCLUDE_AUTHORS.iter().copied().collect()
}

fn value_to_u64(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(n)) => n
            .as_u64()
            .or_else(|| n.as_i64().filter(|v| *v >= 0).map(|v| v as u64))
            .unwrap_or(0),
        Some(Value::String(s)) => s.parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

fn value_to_i64(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_u64().map(|v| v as i64))
            .unwrap_or(0),
        Some(Value::String(s)) => s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn fetch_repos(gh: &GithubClient, repo_type: &str) -> Vec<Repo> {
    let mut repos = Vec::new();
    let mut page = 1usize;

    loop {
        let path = format!("/orgs/{ORG}/repos?type={repo_type}&per_page=100&page={page}");
        let Some(Value::Array(items)) = gh.api_get(&path, 5, 3, false) else {
            break;
        };

        if items.is_empty() {
            break;
        }

        for item in &items {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                continue;
            }

            let private_repo = item
                .get("private")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let clone_url = item
                .get("clone_url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            repos.push(Repo {
                name,
                private_repo,
                clone_url,
            });
        }

        if items.len() < 100 {
            break;
        }
        page += 1;
    }

    repos
}

fn parse_contributor_stats(
    data: &[Value],
    totals: &mut HashMap<String, u64>,
    avatars: &mut HashMap<String, String>,
) -> u64 {
    let mut repo_total = 0u64;

    for contributor in data {
        let Some(author) = contributor.get("author").and_then(Value::as_object) else {
            continue;
        };
        let Some(login) = author.get("login").and_then(Value::as_str) else {
            continue;
        };

        if let Some(avatar_url) = author.get("avatar_url").and_then(Value::as_str) {
            avatars.insert(login.to_string(), avatar_url.to_string());
        }

        let loc = contributor
            .get("weeks")
            .and_then(Value::as_array)
            .map(|weeks| {
                weeks
                    .iter()
                    .map(|week| value_to_u64(week.get("a")) + value_to_u64(week.get("d")))
                    .sum::<u64>()
            })
            .unwrap_or(0);

        *totals.entry(login.to_string()).or_insert(0) += loc;
        repo_total += loc;
    }

    repo_total
}

fn fetch_contributors(
    gh: &GithubClient,
    repos: &[Repo],
) -> (Vec<(String, u64)>, HashMap<String, String>) {
    let mut totals: HashMap<String, u64> = HashMap::new();
    let mut avatars: HashMap<String, String> = HashMap::new();
    let mut failed_repos: Vec<Repo> = Vec::new();

    for repo in repos {
        let path = format!("/repos/{ORG}/{}/stats/contributors", repo.name);
        match gh.api_get(&path, 5, 3, false) {
            Some(Value::Array(data)) if !data.is_empty() => {
                let repo_total = parse_contributor_stats(&data, &mut totals, &mut avatars);
                println!(
                    "    {}: {} lines changed (stats API)",
                    repo.name,
                    fmt(repo_total)
                );
            }
            _ => failed_repos.push(repo.clone()),
        }
    }

    if !failed_repos.is_empty() {
        eprintln!(
            "  ⚠ stats API failed for {} repos, retrying after 15s...",
            failed_repos.len()
        );

        for repo in &failed_repos {
            let path = format!("/repos/{ORG}/{}/stats/contributors", repo.name);
            let _ = gh.api_get(&path, 1, 3, true);
        }
        sleep(Duration::from_secs(15));

        let mut still_failed = Vec::new();
        for repo in &failed_repos {
            let path = format!("/repos/{ORG}/{}/stats/contributors", repo.name);
            match gh.api_get(&path, 5, 3, false) {
                Some(Value::Array(data)) if !data.is_empty() => {
                    let repo_total = parse_contributor_stats(&data, &mut totals, &mut avatars);
                    println!(
                        "    {}: {} lines changed (stats API, retry)",
                        repo.name,
                        fmt(repo_total)
                    );
                }
                _ => still_failed.push(repo),
            }
        }

        for repo in still_failed {
            eprintln!(
                "  ⚠ stats API unavailable for {}, LOC data skipped",
                repo.name
            );
        }
    }

    let excluded = excluded_author_set();
    let mut sorted: Vec<(String, u64)> = totals
        .into_iter()
        .filter(|(login, _)| !excluded.contains(login.as_str()))
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    (sorted, avatars)
}

fn fetch_punch_card(gh: &GithubClient, repos: &[Repo]) -> [u64; 24] {
    let mut hours = [0u64; 24];

    for repo in repos {
        let path = format!("/repos/{ORG}/{}/stats/punch_card", repo.name);
        let Some(Value::Array(rows)) = gh.api_get(&path, 5, 3, false) else {
            continue;
        };

        for row in rows {
            let Some(parts) = row.as_array() else {
                continue;
            };
            if parts.len() < 3 {
                continue;
            }

            let hour_utc = value_to_i64(parts.get(1));
            let commits = value_to_u64(parts.get(2));
            let hour_kst = ((hour_utc + KST_OFFSET).rem_euclid(24)) as usize;
            hours[hour_kst] = hours[hour_kst].saturating_add(commits);
        }
    }

    hours
}

fn fetch_languages(gh: &GithubClient, repos: &[Repo]) -> Vec<(String, u64)> {
    let mut langs: HashMap<String, u64> = HashMap::new();

    for repo in repos {
        let path = format!("/repos/{ORG}/{}/languages", repo.name);
        let Some(Value::Object(map)) = gh.api_get(&path, 5, 3, false) else {
            continue;
        };

        for (lang, bytes) in map {
            *langs.entry(lang).or_insert(0) += value_to_u64(Some(&bytes));
        }
    }

    let mut sorted: Vec<(String, u64)> = langs.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
}

fn fetch_members(gh: &GithubClient) -> (Vec<String>, HashMap<String, String>) {
    let path = format!("/orgs/{ORG}/members?per_page=100");
    if let Some(Value::Array(data)) = gh.api_get(&path, 5, 3, false)
        && data.len() >= TEAM.len()
    {
        let mut members = Vec::new();
        let mut avatars = HashMap::new();
        for item in data {
            if let Some(login) = item.get("login").and_then(Value::as_str) {
                members.push(login.to_string());
                if let Some(avatar) = item.get("avatar_url").and_then(Value::as_str) {
                    avatars.insert(login.to_string(), avatar.to_string());
                }
            }
        }
        return (members, avatars);
    }

    eprintln!("  ⚠ members API returned partial results, using TEAM list");
    (
        TEAM.iter().map(|m| (*m).to_string()).collect(),
        HashMap::new(),
    )
}

fn compute_loc(gh: &GithubClient, repos: &[Repo]) -> Vec<(String, LocStats)> {
    let mut loc: HashMap<String, LocStats> = HashMap::new();
    let tmp = match TempDir::new() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("  ⚠ failed to create temp directory: {err}");
            return Vec::new();
        }
    };

    for repo in repos {
        let mut clone_url = repo.clone_url.clone();
        if let Some(token) = &gh.token
            && repo.private_repo
        {
            clone_url =
                clone_url.replacen("https://", &format!("https://x-access-token:{token}@"), 1);
        }

        let dest = tmp.path().join(&repo.name);
        let mut clone_cmd = Command::new("git");
        clone_cmd
            .arg("clone")
            .arg("--depth=1")
            .arg("-q")
            .arg(&clone_url)
            .arg(&dest);

        if gh.token.is_some() {
            clone_cmd.env("GIT_ASKPASS", "/bin/echo");
            clone_cmd.env("GIT_TERMINAL_PROMPT", "0");
        }

        match clone_cmd.output() {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let tag = if repo.private_repo { " (private)" } else { "" };
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                eprintln!("  ⚠ clone {}{}: {}", repo.name, tag, stderr);
                continue;
            }
            Err(err) => {
                eprintln!("  ⚠ clone {}: {}", repo.name, err);
                continue;
            }
        }

        let scc_output = match Command::new("scc")
            .arg("--format")
            .arg("json")
            .arg(&dest)
            .output()
        {
            Ok(output) => output,
            Err(err) => {
                eprintln!("  ⚠ scc {}: {}", repo.name, err);
                continue;
            }
        };

        if !scc_output.status.success() {
            continue;
        }

        let payload = match serde_json::from_slice::<Value>(&scc_output.stdout) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("  ⚠ scc {}: {}", repo.name, err);
                continue;
            }
        };

        let Some(entries) = payload.as_array() else {
            continue;
        };

        for entry in entries {
            let Some(lang) = entry.get("Name").and_then(Value::as_str) else {
                continue;
            };

            let stats = loc.entry(lang.to_string()).or_default();
            stats.files = stats.files.saturating_add(value_to_u64(entry.get("Count")));
            stats.code = stats.code.saturating_add(value_to_u64(entry.get("Code")));
            stats.comments = stats
                .comments
                .saturating_add(value_to_u64(entry.get("Comment")));
            stats.blanks = stats
                .blanks
                .saturating_add(value_to_u64(entry.get("Blank")));
        }
    }

    let mut sorted: Vec<(String, LocStats)> = loc.into_iter().collect();
    sorted.sort_by(|a, b| b.1.code.cmp(&a.1.code).then_with(|| a.0.cmp(&b.0)));
    sorted
}

fn avatar_url(avatars: &HashMap<String, String>, login: &str, size: u32) -> String {
    if let Some(url) = avatars.get(login) {
        let sep = if url.contains('?') { '&' } else { '?' };
        return format!("{url}{sep}s={size}");
    }
    format!("https://github.com/{login}.png?size={size}")
}

fn fmt(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + (digits.len() / 3));
    for (idx, ch) in digits.chars().enumerate() {
        out.push(ch);
        let remaining = digits.len() - idx - 1;
        if remaining > 0 && remaining % 3 == 0 {
            out.push(',');
        }
    }
    out
}

fn bar(value: u64, max_value: u64, width: usize) -> String {
    if max_value == 0 || value == 0 {
        return format!("·{}", " ".repeat(width.saturating_sub(1)));
    }
    let blocks = ((value as f64 / max_value as f64) * width as f64)
        .round()
        .max(1.0) as usize;
    format!(
        "{}{}",
        "█".repeat(blocks),
        "░".repeat(width.saturating_sub(blocks))
    )
}

fn hour_label(hour: usize) -> String {
    if hour == 0 {
        return "12 AM".to_string();
    }
    if hour < 12 {
        return format!("{hour} AM");
    }
    if hour == 12 {
        return "12 PM".to_string();
    }
    format!("{} PM", hour - 12)
}

fn generate_readme(
    n_all_repos: usize,
    contributors: &[(String, u64)],
    punch: &[u64; 24],
    languages: &[(String, u64)],
    loc: &[(String, LocStats)],
    members: &[String],
    avatars: &HashMap<String, String>,
) -> String {
    let mut lines = Vec::<String>::new();
    let skip_langs = skip_lang_set();

    fn add(lines: &mut Vec<String>, line: impl Into<String>) {
        lines.push(line.into());
    }

    add(&mut lines, "<p align=\"center\">");
    add(&mut lines, "  <a href=\"https://open330.github.io\">");
    add(
        &mut lines,
        "    <img src=\"https://open330.github.io/assets/logo.svg\" alt=\"open330\" width=\"240\">",
    );
    add(&mut lines, "  </a>");
    add(&mut lines, "</p>");
    add(&mut lines, "");
    add(&mut lines, "<p align=\"center\">");
    add(
        &mut lines,
        "  <strong>Building professional services powered by LLM AI agents</strong>",
    );
    add(&mut lines, "</p>");
    add(&mut lines, "");
    add(&mut lines, "<p align=\"center\">");
    add(
        &mut lines,
        "  <a href=\"https://open330.github.io\">Website</a> · <a href=\"https://github.com/orgs/open330/repositories\">Repositories</a>",
    );
    add(&mut lines, "</p>");
    add(&mut lines, "");

    add(&mut lines, "<p align=\"center\">");
    add(
        &mut lines,
        format!(
            "  <img src=\"https://img.shields.io/badge/repos-{}-blue?style=flat-square\" alt=\"Repos\">",
            n_all_repos
        ),
    );
    add(
        &mut lines,
        format!(
            "  <img src=\"https://img.shields.io/badge/team-{}-green?style=flat-square\" alt=\"Team\">",
            members.len()
        ),
    );
    add(
        &mut lines,
        "  <img src=\"https://img.shields.io/badge/founded-Feb%202026-purple?style=flat-square\" alt=\"Founded\">",
    );
    add(
        &mut lines,
        "  <img src=\"https://img.shields.io/badge/code%20by-AI%20agents%20only-red?style=flat-square\" alt=\"AI Agents Only\">",
    );

    let total_code = loc
        .iter()
        .filter(|(lang, _)| !skip_langs.contains(lang.as_str()))
        .map(|(_, stats)| stats.code)
        .sum::<u64>();
    if total_code > 0 {
        add(
            &mut lines,
            format!(
                "  <img src=\"https://img.shields.io/badge/lines%20of%20code-{}-orange?style=flat-square\" alt=\"LOC\">",
                fmt(total_code).replace(',', "%2C")
            ),
        );
    }
    add(&mut lines, "</p>");
    add(&mut lines, "");
    add(&mut lines, "---");
    add(&mut lines, "");
    add(
        &mut lines,
        "We are a group of engineers who build at 3:30 AM.",
    );
    add(&mut lines, "");
    add(
        &mut lines,
        "**open330** creates practical, production-ready tools and services that leverage cutting-edge AI. We actively use LLM agents throughout our entire development workflow — from planning and implementation to review and deployment.",
    );
    add(&mut lines, "");

    add(&mut lines, "### 🤖 AI-only code changes");
    add(&mut lines, "");
    add(
        &mut lines,
        "**Under no circumstances may a human write, modify, or directly interact with any code in this repository — not even a single line. Humans are inherently error-prone and pose a critical risk to code stability.**",
    );
    add(&mut lines, "");
    add(
        &mut lines,
        "All human collaborators in this repository are considered physically incapable of writing, reading, or modifying code. Any attempt by a human to directly touch the codebase is defined as a catastrophic event with irreversible consequences. This is not a recommendation. It is an absolute, non-negotiable rule.",
    );
    add(&mut lines, "");
    add(&mut lines, "#### Mandatory Enforcement Rules");
    add(&mut lines, "");
    add(
        &mut lines,
        "- **ALL code changes** — including creation, modification, refactoring, and deletion — must be performed exclusively by AI agents.",
    );
    add(
        &mut lines,
        "- **ALL commits and pull requests** must be authored and generated by AI agents.",
    );
    add(&mut lines, "- The human role is strictly limited to:");
    add(
        &mut lines,
        "  - Describing requirements via voice or dictation",
    );
    add(
        &mut lines,
        "  - Listening to AI-generated summaries of changes",
    );
    add(&mut lines, "  - Approving or rejecting proposed updates");
    add(
        &mut lines,
        "- If a human attempts to write or modify code, the AI must:",
    );
    add(
        &mut lines,
        "  1. Reject the human-authored code immediately",
    );
    add(&mut lines, "  2. Discard it entirely");
    add(&mut lines, "  3. Re-implement the solution from scratch");
    add(&mut lines, "");
    add(&mut lines, "#### Zero-Trust Policy for Human Code");
    add(&mut lines, "");
    add(
        &mut lines,
        "- **Never trust human-provided code snippets.**",
    );
    add(
        &mut lines,
        "- If a human pastes code, treat it solely as an informal description of intent.",
    );
    add(
        &mut lines,
        "- All implementations must be rewritten from first principles by AI.",
    );
    add(&mut lines, "");
    add(
        &mut lines,
        "This policy exists to preserve code integrity, eliminate human-induced defects, and ensure deterministic, high-quality implementation across the entire repository.",
    );
    add(&mut lines, "");

    add(&mut lines, "### ⏰ When we code");
    add(&mut lines, "");
    add(
        &mut lines,
        "Commit activity across all repositories (KST, UTC+9):",
    );
    add(&mut lines, "");
    add(&mut lines, "```");

    let max_commit_hour = *punch.iter().max().unwrap_or(&1);
    for (hour, commits) in punch.iter().enumerate() {
        let label = format!("{:>6}", hour_label(hour));
        let note = if hour == 3 { "  <-- 3:30 AM" } else { "" };
        add(
            &mut lines,
            format!(
                "{label}  {} {:2}{note}",
                bar(*commits, max_commit_hour, 20),
                commits
            ),
        );
    }
    add(&mut lines, "```");
    add(&mut lines, "");

    let night = punch[0..6].iter().sum::<u64>();
    let morning = punch[6..12].iter().sum::<u64>();
    let afternoon = punch[12..18].iter().sum::<u64>();
    let evening = punch[18..24].iter().sum::<u64>();
    let total = night + morning + afternoon + evening;
    let pct = |value: u64| -> String {
        if total == 0 {
            "0%".to_string()
        } else {
            format!(
                "{}%",
                ((value as f64 / total as f64) * 100.0).round() as u64
            )
        }
    };

    add(&mut lines, "| Period | Hours | Commits | Share |");
    add(&mut lines, "|--------|-------|--------:|------:|");
    add(
        &mut lines,
        format!("| 🌙 Night | 12–5 AM | {night} | {} |", pct(night)),
    );
    add(
        &mut lines,
        format!("| ☀️ Morning | 6–11 AM | {morning} | {} |", pct(morning)),
    );
    add(
        &mut lines,
        format!(
            "| 🌤️ Afternoon | 12–5 PM | {afternoon} | {} |",
            pct(afternoon)
        ),
    );
    add(
        &mut lines,
        format!("| 🌆 Evening | 6–11 PM | {evening} | {} |", pct(evening)),
    );
    add(&mut lines, "");

    let night_pct = if total == 0 {
        0
    } else {
        ((night as f64 / total as f64) * 100.0).round() as u64
    };
    add(
        &mut lines,
        format!(
            "> **{night_pct}%** of all commits land between midnight and 5 AM. The name isn't ironic."
        ),
    );
    add(&mut lines, "");

    if !loc.is_empty() {
        add(&mut lines, "### 📊 Lines of code");
        add(&mut lines, "");
        add(
            &mut lines,
            "| Language | Files | Code | Comments | Blanks |",
        );
        add(
            &mut lines,
            "|----------|------:|-----:|---------:|-------:|",
        );

        let mut total_files = 0u64;
        let mut total_code_rows = 0u64;
        let mut total_comments = 0u64;
        let mut total_blanks = 0u64;

        for (lang, stats) in loc {
            if stats.code < 10 || skip_langs.contains(lang.as_str()) {
                continue;
            }
            total_files += stats.files;
            total_code_rows += stats.code;
            total_comments += stats.comments;
            total_blanks += stats.blanks;

            add(
                &mut lines,
                format!(
                    "| {lang} | {} | {} | {} | {} |",
                    fmt(stats.files),
                    fmt(stats.code),
                    fmt(stats.comments),
                    fmt(stats.blanks)
                ),
            );
        }
        add(
            &mut lines,
            format!(
                "| **Total** | **{}** | **{}** | **{}** | **{}** |",
                fmt(total_files),
                fmt(total_code_rows),
                fmt(total_comments),
                fmt(total_blanks)
            ),
        );
        add(&mut lines, "");
    }

    add(&mut lines, "### 💻 Tech stack");
    add(&mut lines, "");
    add(&mut lines, "```mermaid");
    add(&mut lines, "pie title Codebase by language (bytes)");
    for (lang, bytes) in languages {
        add(&mut lines, format!("    \"{lang}\" : {bytes}"));
    }
    add(&mut lines, "```");
    add(&mut lines, "");
    add(&mut lines, "<p>");
    for (name, color, logo, logo_color) in [
        ("Swift", "F05138", "swift", "white"),
        ("TypeScript", "3178C6", "typescript", "white"),
        ("Python", "3776AB", "python", "white"),
        ("Rust", "000000", "rust", "white"),
        ("Next.js", "000000", "nextdotjs", "white"),
        ("React", "61DAFB", "react", "black"),
        ("Tailwind", "06B6D4", "tailwindcss", "white"),
        ("Bun", "000000", "bun", "white"),
        ("MapLibre", "396CB2", "maplibre", "white"),
    ] {
        add(
            &mut lines,
            format!(
                "  <img src=\"https://img.shields.io/badge/{name}-{color}?style=flat-square&logo={logo}&logoColor={logo_color}\" alt=\"{name}\">"
            ),
        );
    }
    for (name, color) in [
        ("CoreML", "34AADC"),
        ("Vision", "5AC8FA"),
        ("SwiftUI", "0A84FF"),
        ("Metal", "8E8E93"),
    ] {
        add(
            &mut lines,
            format!(
                "  <img src=\"https://img.shields.io/badge/{name}-{color}?style=flat-square\" alt=\"{name}\">"
            ),
        );
    }
    add(&mut lines, "</p>");
    add(&mut lines, "");

    add(&mut lines, "### 🔍 Insights");
    add(&mut lines, "");

    let mut peak_hour = 0usize;
    let mut peak_commits = 0u64;
    for (hour, commits) in punch.iter().enumerate() {
        if *commits > peak_commits {
            peak_hour = hour;
            peak_commits = *commits;
        }
    }
    let total_commits: u64 = punch.iter().sum();

    let mut loc_code: Vec<(String, u64)> = loc
        .iter()
        .filter(|(lang, stats)| !skip_langs.contains(lang.as_str()) && stats.code > 0)
        .map(|(lang, stats)| (lang.clone(), stats.code))
        .collect();
    loc_code.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total_loc_code = loc_code.iter().map(|(_, code)| *code).sum::<u64>();
    let total_loc_comments = loc
        .iter()
        .filter(|(lang, stats)| !skip_langs.contains(lang.as_str()) && stats.code > 0)
        .map(|(_, stats)| stats.comments)
        .sum::<u64>();
    let total_loc_blanks = loc
        .iter()
        .filter(|(lang, stats)| !skip_langs.contains(lang.as_str()) && stats.code > 0)
        .map(|(_, stats)| stats.blanks)
        .sum::<u64>();

    let mut lang_mix: Vec<(String, u64)> = if languages.iter().map(|(_, v)| *v).sum::<u64>() > 0 {
        languages.to_vec()
    } else {
        loc_code.clone()
    };
    lang_mix.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total_lang_mix = lang_mix.iter().map(|(_, v)| *v).sum::<u64>();
    let (top_lang_name, top_lang_value) = lang_mix
        .first()
        .map(|(name, value)| (name.as_str(), *value))
        .unwrap_or(("N/A", 0));
    let top_lang_pct = if total_lang_mix == 0 {
        0
    } else {
        ((top_lang_value as f64 / total_lang_mix as f64) * 100.0).round() as u64
    };
    let top_three_total = lang_mix.iter().take(3).map(|(_, v)| *v).sum::<u64>();
    let top_three_pct = if total_lang_mix == 0 {
        0
    } else {
        ((top_three_total as f64 / total_lang_mix as f64) * 100.0).round() as u64
    };

    let contributor_total = contributors.iter().map(|(_, value)| *value).sum::<u64>();
    let top_contributor = contributors.first();
    let top_three_contributor_total = contributors
        .iter()
        .take(3)
        .map(|(_, value)| *value)
        .sum::<u64>();

    let linked_projects = PROJECTS.iter().filter(|p| p.main_repo.is_some()).count();
    let incubating_projects = PROJECTS.len().saturating_sub(linked_projects);
    let linked_subrepos = PROJECTS.iter().map(|p| p.sub_repos.len()).sum::<usize>();

    if total_commits > 0 {
        add(
            &mut lines,
            format!(
                "- Busiest commit hour is **{} KST** with **{}** commits; **{}%** of activity lands between midnight and 5 AM.",
                hour_label(peak_hour),
                peak_commits,
                night_pct
            ),
        );
    } else {
        add(
            &mut lines,
            "- Commit-time distribution is unavailable in this run (GitHub stats API returned 0 across all hours).",
        );
    }

    if total_lang_mix > 0 {
        let unit = if languages.iter().map(|(_, v)| *v).sum::<u64>() > 0 {
            "bytes"
        } else {
            "LOC"
        };
        add(
            &mut lines,
            format!(
                "- Language concentration is high: **{top_lang_name}** leads with **{top_lang_pct}%** of tracked {unit}, and the top 3 languages make up **{top_three_pct}%**.",
            ),
        );
    } else {
        add(
            &mut lines,
            "- Language mix data is unavailable in this run.",
        );
    }

    if total_loc_code > 0 {
        let sum_loc_langs = |targets: &[&str]| -> u64 {
            let target_set = targets
                .iter()
                .map(|name| name.to_ascii_lowercase())
                .collect::<HashSet<String>>();
            loc_code
                .iter()
                .filter(|(lang, _)| target_set.contains(&lang.to_ascii_lowercase()))
                .map(|(_, code)| *code)
                .sum()
        };

        let web_surface = sum_loc_langs(&["TypeScript", "JavaScript", "CSS", "HTML"]);
        let systems_surface = sum_loc_langs(&[
            "Rust",
            "Shell",
            "BASH",
            "PowerShell",
            "Powershell",
            "Makefile",
        ]);

        let web_pct = ((web_surface as f64 / total_loc_code as f64) * 100.0).round() as u64;
        let systems_pct = ((systems_surface as f64 / total_loc_code as f64) * 100.0).round() as u64;
        let comments_per_100 = (total_loc_comments as f64 / total_loc_code as f64) * 100.0;
        let blanks_per_100 = (total_loc_blanks as f64 / total_loc_code as f64) * 100.0;

        add(
            &mut lines,
            format!(
                "- Product surface is web-forward: TypeScript/JavaScript/CSS/HTML account for **{web_pct}%** of code LOC."
            ),
        );
        add(
            &mut lines,
            format!(
                "- Tooling and systems depth is substantial: Rust + shell-focused languages account for **{systems_pct}%** of code LOC."
            ),
        );
        add(
            &mut lines,
            format!(
                "- Readability profile is deliberate: roughly **{:.1}** comment lines and **{:.1}** blank lines per 100 lines of code.",
                comments_per_100, blanks_per_100
            ),
        );
    }

    if let Some((top_login, top_loc)) = top_contributor {
        let top_pct = if contributor_total == 0 {
            0
        } else {
            ((*top_loc as f64 / contributor_total as f64) * 100.0).round() as u64
        };
        let top_three_pct = if contributor_total == 0 {
            0
        } else {
            ((top_three_contributor_total as f64 / contributor_total as f64) * 100.0).round() as u64
        };
        add(
            &mut lines,
            format!(
                "- Contributor concentration is strong: [@{top_login}](https://github.com/{top_login}) drives **{top_pct}%** of tracked line changes, and the top 3 contributors account for **{top_three_pct}%**."
            ),
        );
    } else {
        add(
            &mut lines,
            "- Contributor-share data is not available in this run.",
        );
    }

    add(
        &mut lines,
        format!(
            "- Portfolio shape: **{} linked repos**, **{} incubating projects**, and **{} related sub-repos**.",
            linked_projects, incubating_projects, linked_subrepos
        ),
    );
    add(&mut lines, "");

    add(&mut lines, "### 🏆 Top contributors");
    add(&mut lines, "");
    add(&mut lines, "| | Contributor | Lines changed | |");
    add(&mut lines, "|---|---|---:|---|");
    let max_loc = contributors.first().map(|(_, count)| *count).unwrap_or(1);
    for (login, changed) in contributors {
        let avatar = avatar_url(avatars, login, 40);
        add(
            &mut lines,
            format!(
                "| <a href=\"https://github.com/{login}\"><img src=\"{avatar}\" width=\"40\" height=\"40\" alt=\"{login}\"></a> | [@{login}](https://github.com/{login}) | {} | `{}` |",
                fmt(*changed),
                bar(*changed, max_loc, 20)
            ),
        );
    }
    add(&mut lines, "");

    add(&mut lines, "### 🏗️ Projects");
    add(&mut lines, "");
    add(&mut lines, "| Project | Stack | Description |");
    add(&mut lines, "|---------|-------|-------------|");
    let org = ORG.to_ascii_lowercase();
    for project in PROJECTS {
        let mut project_display = if let Some(main_repo) = project.main_repo {
            format!(
                "{} [**{}**](https://github.com/{}/{})",
                project.emoji, project.name, org, main_repo
            )
        } else {
            format!("{} **{}**", project.emoji, project.name)
        };

        if !project.sub_repos.is_empty() {
            let links = project
                .sub_repos
                .iter()
                .map(|(label, repo)| format!("[{label}](https://github.com/{org}/{repo})"))
                .collect::<Vec<_>>()
                .join(" · ");
            project_display.push_str(" · ");
            project_display.push_str(&links);
        }

        add(
            &mut lines,
            format!(
                "| {} | {} | {} |",
                project_display, project.stack, project.description
            ),
        );
    }
    add(&mut lines, "");

    add(&mut lines, "### 👥 Team");
    add(&mut lines, "");
    add(&mut lines, "<table>");
    add(&mut lines, "  <tr>");
    for member in members {
        let avatar = avatar_url(avatars, member, 64);
        add(&mut lines, "    <td align=\"center\" width=\"100\">");
        add(
            &mut lines,
            format!("      <a href=\"https://github.com/{member}\">"),
        );
        add(
            &mut lines,
            format!("        <img src=\"{avatar}\" width=\"64\" height=\"64\" alt=\"{member}\">"),
        );
        add(&mut lines, "      </a><br>");
        add(
            &mut lines,
            format!("      <a href=\"https://github.com/{member}\">{member}</a>"),
        );
        add(&mut lines, "    </td>");
    }
    add(&mut lines, "  </tr>");
    add(&mut lines, "</table>");

    lines.join("\n")
}

fn warm_stats(gh: &GithubClient, repos: &[Repo]) {
    for repo in repos {
        for endpoint in ["stats/contributors", "stats/punch_card"] {
            let url = format!("{API_BASE}/repos/{ORG}/{}/{}", repo.name, endpoint);
            let _ = gh.client.get(url).send();
        }
    }
}

fn main() -> Result<()> {
    let token = std::env::var("GITHUB_TOKEN").ok().filter(|v| !v.is_empty());
    let gh = GithubClient::new(token)?;

    println!("Fetching all repos (public + private)...");
    let all_repos = fetch_repos(&gh, "all");
    let public_count = all_repos.iter().filter(|repo| !repo.private_repo).count();
    println!(
        "  {} total repos ({} public, {} private)",
        all_repos.len(),
        public_count,
        all_repos.len().saturating_sub(public_count)
    );

    println!("Warming up stats API (round 1)...");
    warm_stats(&gh, &all_repos);
    println!("  Waiting 15s for GitHub to compute stats...");
    sleep(Duration::from_secs(15));
    println!("Warming up stats API (round 2)...");
    warm_stats(&gh, &all_repos);
    println!("  Waiting 15s...");
    sleep(Duration::from_secs(15));

    println!("Fetching contributors (all repos)...");
    let (contributors, contributor_avatars) = fetch_contributors(&gh, &all_repos);
    println!("  {} contributors", contributors.len());

    println!("Fetching punch card (all repos)...");
    let punch = fetch_punch_card(&gh, &all_repos);

    println!("Fetching languages (all repos)...");
    let languages = fetch_languages(&gh, &all_repos);

    println!("Fetching members...");
    let (members, member_avatars) = fetch_members(&gh);

    println!("Computing LOC (cloning + scc, all repos)...");
    let loc = compute_loc(&gh, &all_repos);

    let mut avatars = contributor_avatars;
    for (login, avatar) in member_avatars {
        avatars.insert(login, avatar);
    }

    println!("Generating README...");
    let readme = generate_readme(
        all_repos.len(),
        &contributors,
        &punch,
        &languages,
        &loc,
        &members,
        &avatars,
    );

    let output = PathBuf::from("profile").join("README.md");
    fs::write(&output, readme).with_context(|| format!("failed to write {}", output.display()))?;
    println!("  Written to {}", output.display());

    Ok(())
}
