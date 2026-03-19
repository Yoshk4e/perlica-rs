# Contributing to Perlica RS

Thank you for considering contributing to Perlica RS.

## Contribution Guidelines

- **One concern per pull request**  
  Fix one bug, add one feature, or clean up one specific area.  
  Avoid mixing unrelated changes (functional fixes + style/whitespace cleanup in the same PR).

- **Smallest possible change**  
  Prefer minimal, focused diffs that are easy to review and reason about.

- **Development branch is the target**  
  Do **not** open pull requests against the `stable` / `main` branch unless it is a critical hotfix (security, crash on startup, etc.).  
  Normal development happens on the `dev` branch.

## Pull Request Process

1. Make sure the project still builds cleanly after your changes  
   ```sh
   cargo build --release
   cargo run --release --bin perlica-game-server
   ```

2. If you added, removed or significantly changed any:
   - command handler
   - protobuf message usage
   - configuration field
   - asset loading logic
   - persistence format  
   → update this `README.md` (especially Configuration and Features sections) and any relevant wiki pages.

3. Write a clear pull request title and description:
   - What problem does this solve?
   - How does it solve it?
   - Any breaking changes?
   - Screenshots / before–after logs (especially for UI/sync related fixes)
   - Which client version was tested?

   Good example titles:
   - `feat: implement CsWeaponAttachGem handler`
   - `fix: prevent negative gem count after detach`
   - `refactor: move breakthrough validation into WeaponDepot`

## Code & Style Expectations

- Follow Rust 2024 edition idioms and `cargo fmt` / `cargo clippy` recommendations
- Prefer explicit over implicit (especially in protobuf handling and bitflag logic)
- Keep functions reasonably short and single-purpose
- Use tracing appropriately (debug for normal flow, info for important events, warn/error for problems)
- Avoid adding new external dependencies unless absolutely necessary
- All new persistent fields must be backward-compatible (use `#[serde(default)]` where sensible)

## Forbidden / Restricted Areas

Certain parts of the codebase are write-protected for most contributors and should only be modified by project maintainers:

- `.github/workflows/*` (CI configuration)
- `Cargo.lock` (unless updating a dependency version is the explicit purpose of the PR)

These restrictions are enforced via branch protection rules and/or CODEOWNERS.

## Intellectual Property & Naming

To reduce legal exposure, please avoid adding or hard-coding:

- Official game names in strings, comments, logs, or variable names
- Real character names, weapon names, skill names, map names when they appear as literal strings
- Any direct asset paths or identifiers that match official naming schemes

Use placeholders, generic terms, or internal identifiers (`chr_XXXX`, `mapXX_lvXXX`, etc.) whenever possible.

Example of acceptable vs problematic:

```rust
// Good
const DEFAULT_SCENE: &str = "map01_lv001";

// Avoid
const DEFAULT_SCENE: &str = "Perlica_Main_City_Alpha";
```

## Questions?

- Open an issue for bugs / feature ideas
- Ask in the Discord server (look for the #development or #contributing channel)

Thank you for helping make Perlica RS better.
