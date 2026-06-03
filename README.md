# airlock

Secure scaffolding for isolated development projects via Dev Containers.

Each project is confined in an OrbStack + VS Code container: dedicated bridge network, `cap-drop=ALL`, secrets mounted read-only.

---

- [airlock](#airlock)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
    - [From source](#from-source)
    - [Via `cargo install`](#via-cargo-install)
  - [Usage](#usage)
    - [Create a new project](#create-a-new-project)
    - [Project location](#project-location)
    - [Retrofit an existing project](#retrofit-an-existing-project)
      - [Monorepos](#monorepos)
      - [Unique UUID prefix](#unique-uuid-prefix)
    - [List available stacks](#list-available-stacks)
  - [Secrets](#secrets)
  - [Generated structure](#generated-structure)

## Prerequisites

- [OrbStack](https://orbstack.dev) (provides the Docker daemon)
- [VS Code](https://code.visualstudio.com) with the `code` CLI in PATH
- `git`
- `age` *(optional — only required for secret encryption)*

## Installation

### From source

```bash
git clone https://github.com/arnaudsene/setup
cd setup
cargo build --release
cp target/release/airlock /usr/local/bin/
```

### Via `cargo install`

```bash
cargo install --path .
```

The `airlock` binary will be available in `~/.cargo/bin/` (make sure this directory is in your `$PATH`).

## Usage

### Create a new project

```bash
airlock new
```

Launches interactive mode: project name, single-stack or monorepo layout, stack
selection, secrets activation.

```bash
airlock new my-project --stack typescript
airlock new my-project --stack rust --secrets
airlock new my-project --stack python --no-git --no-vscode
```

Create a monorepo with one isolated Dev Container per sub-directory:

```bash
airlock new my-app \
  --target frontend=typescript@22:secrets \
  --target backend=python@3.12:no-secrets
```

Target syntax:

```text
--target <subdir>=<stack>[@<version>][:secrets|:no-secrets]
```

The version maps to the stack language version (`node`, `python`, or `rust`).
For example, `frontend=typescript@22` pins Node.js 22 and
`backend=python@3.12` pins Python 3.12. Stacks without language versions, such
as `solidity` and `minimal`, must omit `@<version>`.

In monorepo mode, `--secrets` applies to every target by default and creates a
separate secrets path per sub-directory. Use `:secrets` or `:no-secrets` on a
target to override that default for one sub-project. After setup, airlock opens
the monorepo root in VS Code so you can work across all sub-projects. Reopen a
specific sub-directory in Container when you want to launch its isolated
environment.

Interactive mode also supports monorepos: choose `Monorepo`, then add each
sub-project name, stack, language version, and secrets setting.

Generated Dev Containers install the stack-specific VS Code extensions plus
Claude Code, OpenAI Codex, and Gemini Code Assist.

### Project location

By default the project is created in the **current directory** (`./<slug>`). To
override that, pass one of:

| Flag | Effect |
| --- | --- |
| `-g`, `--global` | Create in `~/Projects/<slug>` (or `$PROJECTS_DIR` if set) |
| `--path <DIR>` | Create in `<DIR>/<slug>` (supports `~` expansion) |

In interactive mode (`airlock new` with no flags), you are prompted to pick
between the current directory, the global projects directory, or a custom path.

### Retrofit an existing project

```bash
airlock retrofit                    # PATH = current directory
airlock retrofit ./my-existing-app
airlock retrofit . --stack rust --secrets
airlock retrofit . --force          # overwrite an existing .devcontainer/
airlock retrofit ./crm              # monorepo: iterate over backend/, frontend/, ...
airlock retrofit ./crm --layout root
airlock retrofit ./crm --layout subprojects
```

Applies the airlock devcontainer config to a project that already exists. It
**does not** touch git (no `init`, no commit), **does not** create a `project/`
folder, and **never** opens VS Code.

#### Monorepos

When airlock detects recognised stacks in immediate sub-directories (depth 1),
it asks which layout to use before generating files:

- `root`: one `.devcontainer/` at the monorepo root.
- `subprojects`: one `.devcontainer/` and secrets path per detected sub-project.

In interactive mode, `subprojects` asks `Y/N` for each detected target, then
per-target secrets. For scripts or CI, pass `--layout root` or
`--layout subprojects`; explicit `subprojects` accepts all detected targets.

Sub-directories whose name starts with `.` (e.g. `.git`, `.venv`) and the
standard build/dependency dirs (`node_modules`, `target`, `dist`, `build`,
`out`, `vendor`, `__pycache__`, `venv`) are skipped. Nested monorepos
(`services/auth/`) remain a per-target invocation (`airlock retrofit services/auth`).

#### Unique UUID prefix

Each retrofit target receives a 4-char hex UUID prefix on its slug, used for
the devcontainer name, container name, and the secrets directory. This avoids
collisions across machines and projects:

- Single project: `~/.airlock/a3f9-myapp/env`
- Monorepo:       `~/.airlock/a3f9-crm-backend/env`, `~/.airlock/b1c2-crm-frontend/env`

On re-runs with `--force`, airlock parses the existing
`.devcontainer/devcontainer.json` to recover the existing UUID prefix so the
secrets directory stays stable. If the existing file has no UUID prefix
(non-airlock or pre-UUID), a new one is generated and a warning is printed
about a potentially orphaned secrets path.

Flags:

| Flag | Effect |
| --- | --- |
| `--stack <id>` | Force the stack (otherwise auto-detected from manifests, with confirmation). Cannot be combined with `--layout subprojects`; use `--layout root` or pass a specific PATH. |
| `--layout <root\|subprojects>` | Choose whether a detected monorepo gets one root `.devcontainer/` or one per detected sub-project |
| `--secrets` | Enable secrets and migrate a root-level `.env` to `~/.airlock/<uuid>-<slug>/env` |
| `--no-secrets` | Disable secrets (useful in non-interactive mode) |
| `--force` | Overwrite existing `.devcontainer/` directories (preserves the UUID prefix) |
| `--no-vscode` | Accepted but no-op (retrofit never opens VS Code) |

Stack detection looks for `Cargo.toml`, `package.json`, `pyproject.toml`,
`foundry.toml`, etc. and proposes a stack — you can confirm or pick another one.

**Secrets migration (with `--secrets`)**: if a `<project>/.env` is present,
airlock moves it into `~/.airlock/<uuid>-<slug>/env` (mode `0600`), ensures
`.env` is in `.gitignore`, and warns loudly if the file was previously
committed to git. If `~/.airlock/<uuid>-<slug>/env` already contains real
entries, no files are touched and you are asked to merge manually.

### List available stacks

```bash
airlock stacks
```

Supported stacks:

| Stack | Description |
| --- | --- |
| `typescript` | Node.js / TypeScript |
| `rust` | Rust |
| `python` | Python |
| `solidity` | Solidity (Foundry) |
| `solidity-ts` | Solidity + TypeScript (Hardhat) |
| `minimal` | Minimal container |

## Secrets

With the `--secrets` flag, a `~/.airlock/<project-name>/env` file is mounted read-only into the container. If `age` is installed, the file can be encrypted using `~/.age/recipients.txt`.

## Generated structure

```text
<target-dir>/<project-name>/
├── .devcontainer/
│   ├── Dockerfile
│   ├── devcontainer.json
│   └── post-create.sh
├── .gitignore
├── SECURITY.md
└── project/             # empty shell — bootstrap your app here
```

Monorepo mode creates a root plus isolated sub-directories:

```text
<target-dir>/<project-name>/
├── .gitignore
├── SECURITY.md
├── frontend/
│   └── .devcontainer/
│       ├── Dockerfile
│       ├── devcontainer.json
│       └── post-create.sh
└── backend/
    └── .devcontainer/
        ├── Dockerfile
        ├── devcontainer.json
        └── post-create.sh
```

Each sub-directory is intentionally left empty except for its `.devcontainer/`;
bootstrap the app directly inside that sub-directory. If you chose to open VS
Code after monorepo creation, airlock opens the root project folder.

Where `<target-dir>` is the current directory by default, `~/Projects` with
`-g`, or whatever you pass to `--path`.

`project/` is left **empty on purpose**. airlock no longer scaffolds a
`package.json`/`Cargo.toml`/`pyproject.toml` for you — instead you bootstrap the
framework of your choice inside the container, in that directory. Because the
folder is empty, generators that refuse to run in a non-empty directory (such as
`create-next-app`) work without conflicts:

```bash
# Inside the container (Reopen in Container), then:
cd project
pnpm create next-app@latest . --yes      # or: cargo init . | uv init | forge init --no-git .
```

If you enabled `--secrets` for a single-stack project, the read-only secrets are
symlinked at `/workspace/.env` (the workspace root). To let an app living in
`project/` pick them up, link them in **after** bootstrapping:

```bash
ln -s ../.env project/.env
```

In monorepo mode, each sub-directory has its own Dev Container. When a target
has secrets enabled, that target's container receives its own `/workspace/.env`.

## System packages

airlock disables runtime `apt`/`apt-get` and `sudo` inside generated
containers via `no-new-privileges`. Add system packages to
`.devcontainer/Dockerfile` before the `USER` line, then rebuild the container:

```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends <package> \
    && rm -rf /var/lib/apt/lists/*
```

In VS Code, run `Dev Containers: Rebuild Container`.
