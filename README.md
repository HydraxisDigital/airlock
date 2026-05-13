# airlock

Secure scaffolding for isolated development projects via Dev Containers.

Each project is confined in an OrbStack + VS Code container: dedicated bridge network, `cap-drop=ALL`, secrets mounted read-only.

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
