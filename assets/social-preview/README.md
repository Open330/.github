# Social preview images

One 1280x640 PNG per public Open330 repository, in the org palette (`#0f1117` background, `#00d4ff` accent, SF Mono / Apple SD Gothic Neo).
They give every repo the same card on Twitter/X, Slack, Discord and the GitHub repo page.

## How to apply (manual, once per repo)

GitHub has no API for the social preview, so each image must be uploaded by hand:

1. Open the repo's **Settings → General** page (links below).
2. Scroll to **Social preview** → **Edit** → **Upload an image…**
3. Pick the matching PNG from this folder and save.

| Repo | Image | Settings |
|---|---|---|
| `.github` | [.github.png](.github.png) | [Settings](https://github.com/Open330/.github/settings) |
| `aas` | [aas.png](aas.png) | [Settings](https://github.com/Open330/aas/settings) |
| `agent-guide` | [agent-guide.png](agent-guide.png) | [Settings](https://github.com/Open330/agent-guide/settings) |
| `agent-quotas` | [agent-quotas.png](agent-quotas.png) | [Settings](https://github.com/Open330/agent-quotas/settings) |
| `agt` | [agt.png](agt.png) | [Settings](https://github.com/Open330/agt/settings) |
| `amux` | [amux.png](amux.png) | [Settings](https://github.com/Open330/amux/settings) |
| `arxiblog` | [arxiblog.png](arxiblog.png) | [Settings](https://github.com/Open330/arxiblog/settings) |
| `barshelf` | [barshelf.png](barshelf.png) | [Settings](https://github.com/Open330/barshelf/settings) |
| `BurstPick-releases` | [BurstPick-releases.png](BurstPick-releases.png) | [Settings](https://github.com/Open330/BurstPick-releases/settings) |
| `BurstPick-web` | [BurstPick-web.png](BurstPick-web.png) | [Settings](https://github.com/Open330/BurstPick-web/settings) |
| `ccusage-fleet` | [ccusage-fleet.png](ccusage-fleet.png) | [Settings](https://github.com/Open330/ccusage-fleet/settings) |
| `context-compress` | [context-compress.png](context-compress.png) | [Settings](https://github.com/Open330/context-compress/settings) |
| `cron-mini-manager` | [cron-mini-manager.png](cron-mini-manager.png) | [Settings](https://github.com/Open330/cron-mini-manager/settings) |
| `docs-sentry` | [docs-sentry.png](docs-sentry.png) | [Settings](https://github.com/Open330/docs-sentry/settings) |
| `homebrew-tap` | [homebrew-tap.png](homebrew-tap.png) | [Settings](https://github.com/Open330/homebrew-tap/settings) |
| `kiwimu` | [kiwimu.png](kiwimu.png) | [Settings](https://github.com/Open330/kiwimu/settings) |
| `muxa` | [muxa.png](muxa.png) | [Settings](https://github.com/Open330/muxa/settings) |
| `open-agent-contribution` | [open-agent-contribution.png](open-agent-contribution.png) | [Settings](https://github.com/Open330/open-agent-contribution/settings) |
| `open330-repo-pulse` | [open330-repo-pulse.png](open330-repo-pulse.png) | [Settings](https://github.com/Open330/open330-repo-pulse/settings) |
| `open330.github.io` | [open330.github.io.png](open330.github.io.png) | [Settings](https://github.com/Open330/open330.github.io/settings) |
| `pr-latest-first` | [pr-latest-first.png](pr-latest-first.png) | [Settings](https://github.com/Open330/pr-latest-first/settings) |
| `quickstart-for-agents` | [quickstart-for-agents.png](quickstart-for-agents.png) | [Settings](https://github.com/Open330/quickstart-for-agents/settings) |
| `travelback` | [travelback.png](travelback.png) | [Settings](https://github.com/Open330/travelback/settings) |

## Regenerating

`generate.py` renders the images with Pillow from a `repos.tsv` (`name<TAB>description`) placed next to it, plus `logo.png` (the org logo from https://open330.github.io/assets/logo.png). It uses macOS system fonts (`SFNSMono.ttf`, `AppleSDGothicNeo.ttc`) so Korean descriptions render correctly.

```sh
gh repo list Open330 --limit 100 --json name,description,isFork,isArchived,visibility \
  --jq '.[] | select(.visibility=="PUBLIC" and .isFork==false and .isArchived==false) | "\(.name)\t\(.description)"' > repos.tsv
python3 generate.py .
```
