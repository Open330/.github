# Contributing to Open330

Thanks for your interest in contributing. These guidelines apply to every repository in the Open330 organization unless a repository provides its own `CONTRIBUTING.md`.

## Getting started

1. **Fork** the repository and clone your fork.
2. Create a **branch** from `main` for your change (e.g. `feat/short-description`, `fix/issue-123`).
3. Make your changes, following the conventions below.
4. Run the repository's **test command** (see its `README.md`, `package.json`, `Cargo.toml`, or `Makefile`) and make sure everything passes.
5. Push your branch and open a **pull request** against `main`.

Small, focused pull requests are easier to review and land faster. For larger changes, open an issue or discussion first so we can agree on the approach.

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): subject
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`, `perf`. The scope is optional. Keep the subject in the imperative mood and under ~72 characters.

Examples:

```
feat(cli): add --dry-run flag
fix(parser): handle empty input without panicking
docs: clarify installation steps
```

## Pull requests

- Fill in the pull request template (summary, test plan, checklist).
- Keep the PR scoped to one logical change; split unrelated work into separate PRs.
- Make sure CI is green. Run the repo's lint, format, and test commands locally before pushing.
- Add or update tests and documentation when behaviour changes.
- Reference related issues (e.g. `Closes #123`).

## Issues

Use the issue templates (bug report / feature request). Include versions, reproduction steps, and expected vs. actual behaviour. For questions, see [SUPPORT.md](SUPPORT.md).

## Releases and publishing

Maintainers handle releases; contributors do not need to bump versions.

- **npm packages** are published under the `@open330/*` scope.
- **Rust crates / binaries** are distributed via GitHub Releases and the Homebrew tap `open330/tap` (`brew install open330/tap/<name>`).

## Security

Do not report security vulnerabilities in public issues. See [SECURITY.md](SECURITY.md) for how to report them privately.

## Code of Conduct

By participating you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Unless stated otherwise, contributions are accepted under the license of the repository you are contributing to.
