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

Launches interactive mode: project name, stack, secrets activation.

```bash
airlock new my-project --stack typescript
airlock new my-project --stack rust --secrets
airlock new my-project --stack python --no-git --no-vscode
```

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
```

Applies the airlock devcontainer config to a project that already exists. It
**does not** touch git (no `init`, no commit), **does not** create a `src/`
folder, and **never** opens VS Code.

#### Monorepos

When the target directory has no recognised manifest at its root (no
`Cargo.toml`, `package.json`, `pyproject.toml`, `foundry.toml`), airlock
falls back to scanning its immediate sub-directories (depth 1). For each
sub-directory with a detected stack it asks `Y/N`, then per-target secrets,
and produces a separate `.devcontainer/` and secrets path for each accepted
target.

Sub-directories whose name starts with `.` (e.g. `.git`, `.venv`) and the
standard build/dependency dirs (`node_modules`, `target`, `dist`, `build`,
`out`, `vendor`, `__pycache__`, `venv`) are skipped. Nested monorepos
(`services/auth/`) remain a per-target invocation (`airlock retrofit services/auth`).

#### Unique UUID prefix

Each retrofit target receives a 4-char hex UUID prefix on its slug, used for
the devcontainer name, container name, and the secrets directory. This avoids
collisions across machines and projects:

- Single project: `~/.secrets/a3f9-myapp/env`
- Monorepo:       `~/.secrets/a3f9-crm-backend/env`, `~/.secrets/b1c2-crm-frontend/env`

On re-runs with `--force`, airlock parses the existing
`.devcontainer/devcontainer.json` to recover the existing UUID prefix so the
secrets directory stays stable. If the existing file has no UUID prefix
(non-airlock or pre-UUID), a new one is generated and a warning is printed
about a potentially orphaned secrets path.

Flags:

| Flag | Effect |
| --- | --- |
| `--stack <id>` | Force the stack (otherwise auto-detected from manifests, with confirmation). In monorepo mode the flag is rejected — pass a specific PATH instead. |
| `--secrets` | Enable secrets and migrate a root-level `.env` to `~/.secrets/<uuid>-<slug>/env` |
| `--no-secrets` | Disable secrets (useful in non-interactive mode) |
| `--force` | Overwrite existing `.devcontainer/` directories (preserves the UUID prefix) |
| `--no-vscode` | Accepted but no-op (retrofit never opens VS Code) |

Stack detection looks for `Cargo.toml`, `package.json`, `pyproject.toml`,
`foundry.toml`, etc. and proposes a stack — you can confirm or pick another one.

**Secrets migration (with `--secrets`)**: if a `<project>/.env` is present,
airlock moves it into `~/.secrets/<uuid>-<slug>/env` (mode `0600`), ensures
`.env` is in `.gitignore`, and warns loudly if the file was previously
committed to git. If `~/.secrets/<uuid>-<slug>/env` already contains real
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

With the `--secrets` flag, a `~/.secrets/<project-name>.env` file is mounted read-only into the container. If `age` is installed, the file can be encrypted using `~/.age/recipients.txt`.

## Generated structure

```text
<target-dir>/<project-name>/
├── .devcontainer/
│   └── devcontainer.json
└── ...                   # files for the chosen stack
```

Where `<target-dir>` is the current directory by default, `~/Projects` with
`-g`, or whatever you pass to `--path`.
