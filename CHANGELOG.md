# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-03-16

### Bug Fixes

- Disable cargo package check for git-only dependency

## [0.1.0-rc.5] - 2026-03-16

### Bug Fixes

- Use per-target forge_args for baseline in test subcommand (#10)
- Correct release-plz action name and split into two jobs (#13)
- Revert changelog_config to workspace key and add release-plz to flake (#15)
- Add persist-credentials: false to release-plz checkout (#16)
- Add git_only to release-plz config
- Exempt .git/ from dotfile gitignore rule

### Features

- Add inspect subcommand and dregs:ignore comments (#11)

## [0.1.0-rc.4] - 2026-03-13

### Bug Fixes

- Add git to nix build inputs and add nix CI job (#8)

### Features

- Improve baseline test output and per-target validation (#9)

## [0.1.0-rc.3] - 2026-03-13

### Bug Fixes

- Use composite actions in example workflow (#6)

### Documentation

- Add features section to CLAUDE.md

### Features

- Add --diff-file flag and enforce pub(crate) visibility (#7)

## [0.1.0-rc.2] - 2026-03-12

### Features

- Add diff-based mutation filter (#5)

## [0.1.0-rc.1] - 2026-03-12

### Documentation

- Add installation instructions for releases and cargo install

### Features

- Add mutr.toml config file for per-target mutation testing (#3)

## [0.1.0-rc.0] - 2026-03-12

### Documentation

- Add rust-dev agent and semantic commit instructions
- Update pre-commit hooks and devShell tools list
- Add crane build plan for nix run support
- Update CLAUDE.md for v0.3 completion
- Update lib.rs description in CLAUDE.md
- Update CLAUDE.md for v0.4 parallel execution and CI sharding

### Features

- Initialize Cargo project with dependencies
- Add module structure with generator trait, runner, and report
- Implement GambitGenerator with gambit library integration
- Implement test runner with forge subprocess
- Complete MVP with CLI, reporting, and test isolation
- Add nix package with crane for `nix run` support
- Parse foundry.toml for project config and auto-detect project root
- Resolve remappings via forge and add progress output
- Run baseline tests before mutation testing
- List matched tests before baseline run
- Show mutation diff for surviving mutants
- Parallel execution and CI sharding (v0.4)
- Add GitHub Actions CI/CD and markdown report format (#1)

### Doc

- Add README

