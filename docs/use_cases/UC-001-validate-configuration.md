# Use Case: Validate Configuration

## Overview

**Use Case ID:** UC-001  
**Use Case Name:** Validate Configuration  
**Primary Actor:** User  
**Goal:** Confirm that the Clockify and Solidtime configuration is complete and usable before running migration or comparison commands.  
**Status:** Approved

## Preconditions

- User can run the command-line application.
- User has access to required configuration values through supported configuration sources.
- Clockify and Solidtime are expected to be available for account checks.

## Configuration Reference

The command form is:

```sh
clockify-to-solidtime validate [--config <path>]
```

### Configuration Values

| Configuration value       | Required | Environment variable        | Config file key             | Default                            |
|---------------------------|----------|-----------------------------|-----------------------------|------------------------------------|
| Clockify credential       | Yes      | `CLOCKIFY_API_KEY`          | `clockify_api_key`          | -                                  |
| Solidtime credential      | Yes      | `SOLIDTIME_API_TOKEN`       | `solidtime_api_token`       | -                                  |
| Clockify workspace        | No       | `CLOCKIFY_WORKSPACE_ID`     | `clockify_workspace_id`     | User's default workspace           |
| Solidtime organization    | No       | `SOLIDTIME_ORGANIZATION_ID` | `solidtime_organization_id` | User's only available organization |
| Clockify service address  | No       | `CLOCKIFY_BASE_URL`         | `clockify_base_url`         | `https://api.clockify.me/api/v1`   |
| Solidtime service address | No       | `SOLIDTIME_BASE_URL`        | `solidtime_base_url`        | `https://app.solidtime.io/api`     |

### Supported Configuration Sources

Configuration values are read from the following sources, in precedence order:

1. Value in the file passed with `--config <path>`.
2. Exported environment variable.
3. Value from a `.env` file in the working directory.
4. Built-in default, when the value has one.

The user does not have to select a source. The system reads the available sources and applies this precedence automatically. The `--config` option is only needed when values live in a config file.

## Main Success Scenario

1. User runs the `validate` command, optionally providing a configuration file with `--config`.
2. System reads the available configuration values.
3. System checks that the required Clockify configuration is present.
4. System checks that the required Solidtime configuration is present.
5. System confirms access to the configured Clockify account and workspace.
6. System confirms access to the configured Solidtime account and organization.
7. System displays a successful validation summary, including the confirmed source workspace and target organization, and confirms that no migration data was changed.

## Alternative Flows

### A1: Configuration Source Cannot Be Loaded

**Trigger:** A configuration source cannot be read or parsed (step 2)
**Flow:**

1. System displays a specific error message naming the configuration source that cannot be loaded.
2. System does not continue with account checks.
3. Use case ends.

### A2: Required Clockify Configuration Is Missing

**Trigger:** Required Clockify configuration is missing or empty (step 3)
**Flow:**

1. System displays a specific error message naming the missing Clockify configuration value.
2. System does not continue with Clockify account checks.
3. Use case ends.

### A3: Required Solidtime Configuration Is Missing

**Trigger:** Required Solidtime configuration is missing or empty (step 4)
**Flow:**

1. System displays a specific error message naming the missing Solidtime configuration value.
2. System does not continue with Solidtime account checks.
3. Use case ends.

### A4: Clockify Access Cannot Be Confirmed

**Trigger:** Clockify rejects the configured access or cannot be reached (step 5)
**Flow:**

1. System displays a specific error message naming Clockify as the failed service.
2. System explains whether the failure relates to account access, workspace access, or service availability when that distinction is known.
3. Use case ends.

### A5: Clockify Workspace Cannot Be Determined

**Trigger:** The configured Clockify account does not identify a usable workspace (step 5)
**Flow:**

1. System displays a specific error message explaining that a Clockify workspace must be selected.
2. System does not continue with Solidtime account checks.
3. Use case ends.

### A6: Solidtime Access Cannot Be Confirmed

**Trigger:** Solidtime rejects the configured access or cannot be reached (step 6)
**Flow:**

1. System displays a specific error message naming Solidtime as the failed service.
2. System explains whether the failure relates to account access, organization access, or service availability when that distinction is known.
3. Use case ends.

### A7: Solidtime Organization Cannot Be Determined

**Trigger:** The configured Solidtime account does not identify exactly one usable organization (step 6)
**Flow:**

1. System displays a specific error message explaining that a Solidtime organization must be selected.
2. System does not report validation success.
3. Use case ends.

## Postconditions

### Success Postconditions

- Clockify configuration has been confirmed as usable for the selected source workspace.
- Solidtime configuration has been confirmed as usable for the selected target organization.
- No Clockify data, Solidtime data, or local migration state has been changed.

### Failure Postconditions

- Configuration has not been accepted as ready for migration or comparison.
- No Clockify data, Solidtime data, or local migration state has been changed.
- User has received at least one specific error message naming the failed setup step or service.

## Business Rules

### BR-001: Required Service Credentials

Clockify and Solidtime access credentials must be present before the system attempts account validation.

### BR-002: Optional Configuration Source

The user may provide a configuration source, but validation can proceed without one when required configuration values are available from other supported sources.

### BR-003: Clockify Workspace Selection

Validation succeeds only when the system can determine one Clockify source workspace from the configured workspace or the user's default workspace.

### BR-004: Solidtime Organization Selection

Validation succeeds only when the system can determine one Solidtime target organization from the configured organization or the user's available memberships.

### BR-005: Read-Only Validation

Validation must not create, update, or delete Clockify data, Solidtime data, or local migration state.

### BR-006: Specific Failure Output

Failed validation must identify the failed setup step or service clearly enough for the user to decide what configuration to fix.

### BR-007: Configuration Source Precedence

When a configuration value is available from more than one source, a value from the file passed with `--config` takes precedence over an exported environment variable, which takes precedence over a value from a `.env` file.
