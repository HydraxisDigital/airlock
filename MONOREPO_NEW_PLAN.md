# Plan: monorepo creation for `airlock new`

## Goal

Allow `airlock new` to create a monorepo with multiple isolated sub-projects,
for example:

```bash
airlock new crm \
  --target frontend=typescript@22 \
  --target backend=python@3.12
```

Expected result:

```text
crm/
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

Each sub-directory should be independently openable in VS Code with its own
Dev Container, stack version, container name, and optional secrets path.

## Status

Initial implementation started:

- non-interactive `--target <subdir>=<stack>[@<version>]` support is in place
- interactive mode can now choose `Single stack` or `Monorepo`
- interactive monorepo mode prompts for each sub-project name, stack, language
  version, and secrets setting
- monorepo scaffolding writes one `.devcontainer/` per target sub-directory
- `--secrets` applies to every target and creates per-target mounts
- CLI target specs support `:secrets` and `:no-secrets` per target
- end-to-end CLI tests cover successful and invalid monorepo creation
- integration tests cover structure, target versions, secrets mounts, duplicate
  targets, and invalid versions on non-versionable stacks

Still pending:

- manual interactive UX pass in a real terminal

## Current State

`airlock new` currently creates one project directory with:

- one `.devcontainer/`
- one stack
- one language version
- one `project/` empty shell
- optional git init
- optional secrets setup

`airlock retrofit` already has useful monorepo behavior:

- it scans immediate sub-directories with `detect_substacks`
- it creates one `.devcontainer/` per detected sub-project
- it generates unique UUID-prefixed slugs
- it keeps secrets separate per target

The new feature should reuse this existing target-oriented behavior instead of
introducing a parallel implementation.

## Proposed UX

### Non-interactive mode

Start with a compact repeatable `--target` flag:

```bash
airlock new crm \
  --target frontend=typescript@22 \
  --target backend=python@3.12
```

Suggested target grammar:

```text
<subdir>=<stack>[@<version>]
```

Examples:

```text
frontend=typescript@22
backend=python@3.12
worker=rust@1.94.0
contracts=solidity
```

The `@<version>` value maps to the stack language version:

- `typescript`, `javascript`, `solidity-ts`: Node.js version
- `python`: Python version
- `rust`: Rust version
- stacks without language versions reject `@<version>`

### Interactive mode

When running:

```bash
airlock new
```

Prompt for layout:

```text
Project layout?
> Single stack
  Monorepo
```

For monorepos, repeat:

```text
Sub-project name: frontend
Stack: TypeScript / Node.js
Node version: 22
Configure secrets for frontend? no
Add another sub-project? yes
```

Then:

```text
Sub-project name: backend
Stack: Python
Python version: 3.12
Configure secrets for backend? yes
Add another sub-project? no
```

## Data Model

Add a target-level config:

```rust
pub struct NewTargetConfig {
    pub name: String,
    pub stack: StackChoice,
    pub needs_secrets: bool,
}
```

Add a layout enum:

```rust
pub enum NewProjectLayout {
    Single(ProjectConfig),
    Monorepo {
        name: String,
        location: PathBuf,
        init_git: bool,
        open_vscode: bool,
        targets: Vec<NewTargetConfig>,
    },
}
```

This keeps the existing single-project config clean and avoids forcing monorepo
concerns into `ProjectConfig`.

## Implementation Steps

1. Add CLI support for repeated `--target`.

   In `src/main.rs`, extend `Commands::New` with:

   ```rust
   #[arg(long, value_name = "SUBDIR=STACK[@VERSION]")]
   target: Vec<String>,
   ```

   Reject combining `--target` with single-stack-only version flags such as
   `--node-version`, `--python-version`, and `--rust-version`, unless we later
   define clear precedence rules.

2. Parse target specs.

   Add a parser, likely in `src/cli.rs` or a small new module:

   ```rust
   fn parse_new_target_spec(spec: &str) -> Result<ParsedNewTarget>
   ```

   Validate:

   - subdir name slugifies to a non-empty value
   - subdir does not contain path traversal
   - stack exists in `StackRegistry`
   - version is supported by the stack language, if provided
   - version is rejected for stacks without `language_version`
   - duplicate subdir names are rejected

3. Resolve target stack versions.

   Reuse the existing version validation path around `LanguageVersion` and
   `StackChoice::with_version`.

   Some of the current helper functions in `src/cli.rs` may need to become
   reusable:

   - `stack_meta_for`
   - `validate_supported`
   - `resolve_language_version`

4. Add monorepo scaffolding.

   Add a new function, probably in `src/scaffold.rs`:

   ```rust
   pub fn run_monorepo(
       root_paths: &ProjectPaths,
       targets: &[NewTargetConfig],
       opts: &ScaffoldOptions,
       secrets_dir: &Path,
   ) -> Result<()>
   ```

   It should:

   - create the root directory
   - create each target subdir
   - write `.devcontainer/` files inside each target subdir
   - write root-level `.gitignore`
   - write root-level `SECURITY.md`
   - set up per-target secrets when requested
   - initialize git only at the monorepo root
   - open VS Code at the monorepo root or print guidance for opening subdirs

5. Reuse retrofit path conventions.

   For each subdir:

   ```rust
   let base_slug = slug_for_subdir(&root_paths.dir, subdir_name);
   let uuid = new_uuid();
   let paths = ProjectPaths::for_existing_with_slug(
       subdir_path,
       secrets_dir,
       &uuid,
       &base_slug,
   )?;
   ```

   This keeps secrets paths aligned with retrofit:

   ```text
   ~/.airlock/a3f9-crm-frontend/env
   ~/.airlock/b1c2-crm-backend/env
   ```

6. Avoid `project/` in monorepo subdirs.

   For single-stack `new`, keep the current `project/` behavior unchanged.

   For monorepo `new`, each subdir should itself be the empty application shell.
   This lets users run:

   ```bash
   cd frontend
   pnpm create next-app@latest . --yes

   cd ../backend
   uv init
   ```

7. Improve summaries and next steps.

   Add a monorepo summary similar to retrofit:

   ```text
   crm is ready.

   frontend/   typescript 22   a3f9-crm-frontend
   backend/    python 3.12     b1c2-crm-backend

   Next steps:
   1. Open a subdir in VS Code and choose "Reopen in Container"
   2. Bootstrap each app in its subdir
   3. Edit secrets at the paths shown above
   ```

8. Add tests.

   Integration tests should cover:

   - existing single-stack `new` behavior remains unchanged
   - monorepo root is created
   - each target subdir has `.devcontainer/Dockerfile`
   - each target subdir has `.devcontainer/devcontainer.json`
   - TypeScript target pins `typescript-node:22`
   - Python target pins `python:3.12`
   - monorepo subdirs do not contain `project/`
   - git is initialized only at the root when enabled
   - secrets paths are distinct per target
   - invalid target specs fail clearly
   - duplicate target names fail clearly
   - version on non-versionable stacks fails clearly

## Open Product Decisions

- Should `--secrets` enable secrets for every target, or should target specs
  eventually allow per-target secrets?

  Initial recommendation: `--secrets` applies to all targets. Interactive mode
  can ask per target.

- Should VS Code open the root or the first target?

  Initial recommendation: open the root and print explicit guidance that each
  subdir has its own Dev Container. Opening a specific subdir may be surprising
  when there are multiple targets.

- Should root-level `.devcontainer/` exist for monorepos?

  Initial recommendation: no. Keep isolation explicit and per sub-project,
  matching the existing retrofit monorepo behavior.

## Suggested Delivery Order

1. Implement non-interactive `--target`.
2. Add parser and integration tests.
3. Add monorepo scaffold runner.
4. Update README.
5. Add interactive monorepo flow.

This keeps the first usable slice small while preserving a clear path toward a
nice interactive experience.
