# Use Case: Generate Shell Completions

## Overview

**Use Case ID:** UC-004  
**Use Case Name:** Generate Shell Completions  
**Primary Actor:** User  
**Goal:** Generate a shell completion script so commands and options can be completed by the user's shell.  
**Status:** Approved

## Preconditions

- User can run the command-line application.
- User knows which supported shell needs a completion script.

## Command Reference

The command form is:

```sh
clockify-to-solidtime completions <shell>
```

Supported shell values are `bash`, `zsh`, `fish`, `powershell`, and `elvish`.

This use case does not require service credentials, a configuration file, network access, or a `.env` file.

## Main Success Scenario

1. User runs the `completions` command with a supported shell value.
2. System determines the command and option structure for the application.
3. System writes the completion script for the selected shell to standard output.
4. User receives the completion script and can install it for the selected shell, achieving the goal.

## Alternative Flows

### A1: Shell Argument Is Missing

**Trigger:** User runs the `completions` command without a shell value (step 1)
**Flow:**

1. System displays a usage error that identifies the missing shell argument.
2. System exits with a non-zero status.
3. Use case ends.

### A2: Shell Argument Is Unsupported

**Trigger:** User runs the `completions` command with an unsupported shell value (step 1)
**Flow:**

1. System displays a usage error that identifies the supported shell values.
2. System exits with a non-zero status.
3. Use case ends.

## Postconditions

### Success Postconditions

- A completion script for the selected shell has been written to standard output.
- No Clockify data, Solidtime data, configuration file, `.env` file, or local migration state has been changed.

### Failure Postconditions

- No completion script has been emitted for installation.
- User has received a specific usage error.
- No Clockify data, Solidtime data, configuration file, `.env` file, or local migration state has been changed.

## Business Rules

### BR-019: Supported Completion Shells

Completion generation supports bash, zsh, fish, powershell, and elvish.

### BR-020: Offline Completion Generation

Completion generation must not require service credentials, configuration validation, network access, or a readable `.env` file.

### BR-021: Read-Only Completion Generation

Completion generation must not create, update, or delete Clockify data, Solidtime data, configuration files, `.env` files, or local migration state.
