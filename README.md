# 👀 peek

> A friendly, casual TUI dashboard to take a quick peek at your projects, git activity, and code statistics.

Built with 🦀 **Rust**, **Ratatui**, and **Crossterm**.

---

## ✨ Features

- 📂 **Multi-Project Scanner**: Scans your development workspace (e.g. `~/Dev`) and extracts LOC, programming languages, and git repositories.
- 🧹 **Clean Language Filtering**: Filters out non-programming files (HTML, CSS, JSON, Markdown, YAML, TOML, etc.) so your stats reflect pure programming logic.
- 📅 **52-Week Activity Heatmap**: Interactive GitHub-style commit activity heatmap for each project and an aggregated global heatmap across all projects.
- 🔍 **Technical Debt Scanner**: Detects and highlights `TODO` and `FIXME` comments.
- 🐙 **GitHub Integration**: Fetches public commit events and recent public repositories via GitHub REST API.
- 🙈 **Ignore Mock / Experiment Projects**: Configurable `ignored_projects` list to hide sandbox or test projects with toggle (`i`).

---

## 🚀 Quick Start

### Installation & Build

```bash
# Clone the repository
git clone https://github.com/yusqohid/peek.git
cd peek

# Build with Cargo
cargo build --release
```

The compiled binary will be located at `target/release/peek`.

### Usage

```bash
# Launch peek scanning current or default directory (~/Dev)
peek

# Scan a specific directory
peek --path ~/Dev

# Scan with custom depth and GitHub username
peek --path ~/Dev --depth 2 --github-user yourusername
```

---

## ⌨️ Keybindings

| Key | Action |
|---|---|
| `1` | Dashboard tab |
| `2` | Projects tab |
| `3` | GitHub Activity tab |
| `4` / `?` | Help tab |
| `Tab` / `Shift+Tab` | Next / Previous tab |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` | Open selected project detail view |
| `Esc` / `Backspace`| Return to project list from detail |
| `s` | Cycle sort order (Name → LOC → Commits → Recent) |
| `d` | Toggle sort direction (ascending / descending) |
| `/` | Filter projects by name (`Enter` apply, `Esc` clear) |
| `g` / `Home` | Go to top |
| `G` / `End` | Go to bottom |
| `i` | Toggle ignored projects visibility |
| `r` | Rescan workspace and refresh GitHub data |
| `q` / `Ctrl+C` | Quit |

---

## ⚙️ Configuration

Copy `config.example.toml` to `config.toml` to customize settings:

```toml
[general]
scan_directory = "~/Dev"
scan_depth = 3
refresh_interval_secs = 300  # background auto-refresh; 0 disables it

[display]
default_tab = "dashboard"  # dashboard | projects | github | help

[analysis]
exclude_dirs = ["node_modules", "target", "dist", "build", ".git", ".venv", "venv"]
ignored_projects = ["mock-project", "temp-experiment"]

[github]
username = "yourusername"
# token = "ghp_yourToken" # or set via GITHUB_TOKEN environment variable
```

> ⚠️ `config.toml` is git-ignored — never commit it with a real token inside.
> Prefer the `GITHUB_TOKEN` environment variable over storing a token in the file.

---

## 🖥️ Terminal & Font Requirements

- A terminal with Unicode support. Project types, activity, and GitHub events
  render with emoji (🦀 📦 🐍 …), so no Nerd Font is required.
- If glyphs show as boxes, switch the terminal font to one with good emoji
  coverage (e.g. Noto Color Emoji) or a Nerd Font.
- Narrow terminals (< 80 columns) automatically get a compact project table.

---

## 🐙 GitHub Data Notes

- Only **public** data is read via the unauthenticated REST API:
  user profile, public events (`/events/public`), and public repos.
- Rate limits apply (60 req/hour without a token). Set `GITHUB_TOKEN`
  (or `[github] token`) to raise the quota; when the limit is hit the
  GitHub tab shows a rate-limit error with a retry hint.
- The activity feed covers roughly the last 30 days of public events.

---

## 📄 License

MIT License
