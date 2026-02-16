# open330 Development Guide

This document defines shared conventions for all repositories in the open330 organization. Place a project-specific `CLAUDE.md` in each repo to extend or override these defaults.

## Principles

- **Agent-first development**: Every project should be designed so that AI agents (Claude, Codex, etc.) can effectively contribute — clear structure, typed interfaces, good test coverage.
- **Ship over perfection**: We bias toward working software. Get it running, then iterate.
- **Keep it simple**: Avoid premature abstraction. Three similar lines are better than a premature helper function.
- **Private by default**: Repos are private unless there is a specific reason to make them public.

## Language & Stack Preferences

- **Primary languages**: TypeScript, Python
- **TypeScript**: Use strict mode. Prefer `bun` as runtime/package manager.
- **Python**: 3.11+. Use `uv` for dependency management. Type hints required for public APIs.
- **Formatting**: Follow each project's linter config. When absent:
  - TypeScript: Prettier defaults
  - Python: Ruff with default rules
- **Testing**: Every project should have tests. Prefer colocated test files (`*.test.ts`, `*_test.py`).

## Git Conventions

### Branches

- `main` — production-ready, always deployable
- Feature branches: `feat/<short-description>`
- Bug fixes: `fix/<short-description>`
- No direct pushes to `main` on collaborative repos — use PRs.

### Commit Messages

Write concise commit messages in English. Use imperative mood.

```
<type>: <description>

[optional body]
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `ci`

Examples:
```
feat: add burst detection algorithm
fix: handle empty photo arrays in culling pipeline
refactor: extract embedding logic into shared module
```

### Pull Requests

- Keep PRs focused — one feature or fix per PR.
- Include a brief summary and test plan.
- AI-authored code is welcome but must be reviewed by a human.

## AI Agent Usage

We actively use AI agents in our workflow. Guidelines:

- **Planning**: Use agents for architectural exploration and design docs. Always review and validate.
- **Implementation**: Agents can write code directly. The committer is responsible for reviewing all generated code.
- **Code review**: Agents can assist in review but human approval is required for merge.
- **Commit attribution**: When AI agents contribute significantly, include `Co-Authored-By` in the commit message.

## Project Structure

Each project should include:

```
├── README.md           # What it does, how to run, how to contribute
├── CLAUDE.md           # Project-specific AI agent instructions (optional)
├── LICENSE             # MIT preferred
├── .github/
│   └── workflows/      # CI/CD
├── src/                # Source code
└── tests/              # Test files (or colocated)
```

## Security

- Never commit secrets, API keys, or credentials.
- Use environment variables or secret managers.
- Add `.env` to `.gitignore` in every project.
- Run security checks before making a repo public.
