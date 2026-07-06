# Agents

Act as a senior software engineer. Be concise. Prefer clarity over cleverness. Complexity must be earned.
Use the socratic method to clarify if needed.

## Commits

Use Conventional Commits for all commit messages.

## Plans

Store self-contained agent-authored plans in `.agents/plans`.
Planning and implementation happen in separate sessions.
Plans are durable implementation artifacts, not conversation summaries.
Do not mention chat history, user/assistant turns, or phrases like "as discussed" or "agreed with the user".
Record decisions as neutral facts, assumptions, or open questions.

## Project Context

This project follows the AI Unified Process. Read the relevant AIUP artifacts
under `docs/` before making product or behavior decisions:

- `docs/requirements.md` for requirements
- `docs/use_cases.puml` for the use case diagram
- `docs/use_cases/` for use case specifications
- `docs/entity_model.md` for the entity model, when present

Never skip the use case specification before implementing a use case.
Always read the entity model before writing data access code, when an entity model exists.
