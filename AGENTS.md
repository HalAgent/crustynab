# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with 
code in this repository.

## Style

Try to keep the style as functional as possible ("Ocaml with manual garbage 
collection", as opposed to "C++ with borrow checker"). Use features like 
Algebraic Data Types and Traits liberally, with an algebra-oriented design 
mindset

When writing new documentation files, ensure to clarify that "Documentation written 
by Claude Code" somewhere in the file.

ALL tests should be in the `tests/` directory, and should follow the testing
instructions in the `## Testing` section.

This project is in heavy development. Whenever you make a change, make sure to 
check `CLAUDE.md` and update it if necessary to reflect any newly added/changed 
features or structures

## Error Handling & Safety Guidelines

### Never Use `unwrap()` in Production Code
- **NEVER** use `.unwrap()` on `Option` or `Result` types in production paths
- Use proper error handling with `?`, `.ok_or()`, `.map_err()`, or pattern matching
- Example: Replace `tag_name.chars().nth(1).unwrap()` with proper error handling
- Exception: Only use `unwrap()` in tests or when preceded by explicit checks that guarantee safety

### Error Message Quality
- Include contextual information in error messages
- Use structured error types instead of plain strings where possible
- Provide actionable information for debugging

## Project Structure

This is a Rust port of `budget-utils` (Python). It fetches YNAB budget data and
generates weekly spending reports in multiple output formats.

### Modules

- `src/config.rs` — Configuration types (`Config`, `OutputFormat`) and JSON loading
- `src/calendar_weeks.rs` — Sunday–Saturday week partitioning split at month boundaries
- `src/ynab.rs` — YNAB API types, `YnabApi` trait, and `HttpYnabClient` adapter over `ynab-api`
- `src/report.rs` — Polars DataFrame transforms: `CategoryFrame`, `TransactionFrame`,
  `build_report_table`, `build_category_group_totals_table`
- `src/visual_report.rs` — HTML report generation with interactive table selection
- `src/main.rs` — CLI entry point (`clap`) and orchestration via `run(api, config)`

### Key Dependencies

- `polars` (lazy, csv, fmt, dtype-date, is_in) — DataFrame operations
- `ynab-api` — YNAB REST API client bindings used by `HttpYnabClient`
- `chrono` — Date handling
- `indexmap` — Ordered maps for category group watch list
- `serde` / `serde_json` — Config deserialization
- `anyhow` — Error handling
- `html-escape` — HTML escaping in visual reports

### Configuration

The program reads `config.json` (path configurable via `-c`/`--config`). Fields:
- `budgetName`, `personalAccessToken`, `categoryGroupWatchList` (ordered map of group→hex color)
- `resolution_date` (optional, defaults to today), `showAllRows`, `outputFormat`
- Output formats: `"polars_print"`, `"csv_print"`, `{"csv_output": "path"}`, `{"visual_output": "path"}`

## Development Environment

This project uses Nix for reproducible builds and development environments. The
`flake.nix` provides all necessary dependencies. You are always running in the relevant nix environment.

## Testing

The project uses a **mixed testing model**:
- **Snapshot testing** via `insta` for stable user-facing outputs (CLI/text/HTML tables, goldens)
- **Property-based testing** via `proptest` for invariants and algebraic/data-shape behavior

### Snapshot Testing Approach

Snapshot tests follow these principles:
- **Single assertion per test**: Each test has exactly one `insta::assert_snapshot!()` or `insta::assert_json_snapshot!()` call
- **Deterministic snapshots**: Dynamic data (timestamps, file sizes, temp paths) is normalized to ensure reproducible results
- **Literal value snapshots**: Snapshots contain only concrete, expected values without variables
- **Offline resilience**: All tests must pass in offline environments (CI, restricted networks) by using dual-snapshot patterns or graceful degradation

 in `tests/golden_output/`

### Property Testing Approach

Property tests should:
- Encode invariants (coverage, conservation, ordering, set-difference, aggregation correctness)
- Use bounded generators so default `cargo test` remains practical
- Prefer deterministic comparisons when checking tabular outputs (normalize order or compare multisets)

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test <test_name>

# Review and accept snapshot changes
cargo insta review

# Auto-accept all snapshot changes (use carefully)
cargo insta accept
```

### Snapshot Management

- Snapshots are stored in `src/snapshots/` (unit tests) and `tests/snapshots/` (integration tests)
- When test behavior changes, run `cargo insta review` to inspect differences
- Accept valid changes with `cargo insta accept` or reject with `cargo insta reject`
- Never commit `.snap.new` files - these are pending snapshot updates

## Version control

This project uses jujutsu `jj` for version control
