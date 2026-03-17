# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2026-03-17

### Features

- Add custom test command support (#22)
- Add flake overlay and template (#23)

## [0.1.1] - 2026-03-16

### Bug Fixes

- Disable cargo package check for git-only dependency
- Isolate git commands from parent process env

### Documentation

- Fix release instructions

### Features

- Add exclude_functions to dregs.toml (#18)

## [0.1.0] - 2026-03-16

### Bug Fixes

- Use composite actions in example workflow (#6)
- Add git to nix build inputs and add nix CI job (#8)
- Use per-target forge_args for baseline in test subcommand (#10)
- Correct release-plz action name and split into two jobs (#13)
- Revert changelog_config to workspace key and add release-plz to flake (#15)
- Add persist-credentials: false to release-plz checkout (#16)
- Add git_only to release-plz config
- Exempt .git/ from dotfile gitignore rule

### Documentation

- Add rust-dev agent and semantic commit instructions
- Update pre-commit hooks and devShell tools list
- Add crane build plan for nix run support
- Update CLAUDE.md for v0.3 completion
- Update lib.rs description in CLAUDE.md
- Update CLAUDE.md for v0.4 parallel execution and CI sharding
- Add installation instructions for releases and cargo install
- Add features section to CLAUDE.md

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
- Add mutr.toml config file for per-target mutation testing (#3)
- Add diff-based mutation filter (#5)
- Add --diff-file flag and enforce pub(crate) visibility (#7)
- Improve baseline test output and per-target validation (#9)
- Add inspect subcommand and dregs:ignore comments (#11)

### Doc

- Add README

