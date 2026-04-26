# CLAUDE.md

This file tells AI coding agents which project documents to read before making
changes. Prefer reading the smallest relevant document instead of loading a
large guide end-to-end.

## Product Context

- For project explanation, product scope, requirements, user flows, MVP
  decisions, or behavior that needs product context, read `spec/prd.md`.

## Frontend Code Rules

Frontend guidance is split under `spec/code/frontend/`. Read the relevant file
for the task:

- When creating or editing React components, pages, routes, or event handlers,
  read `spec/code/frontend/function-style.md`.
- When adding or changing styles, CSS, class names, layout, spacing, or theme
  usage, read `spec/code/frontend/styling.md`.
- When simplifying JSX, conditions, constants, ternaries, or render branches,
  read `spec/code/frontend/readability.md`.
- When extracting components, changing component boundaries, removing props
  drilling, or adding guards/interaction components, read
  `spec/code/frontend/component-design.md`.
- When adding hooks, API helpers, validation functions, services, wrappers, or
  functions with side effects, read `spec/code/frontend/predictability.md`.
- When changing folder structure, feature boundaries, state ownership, shared
  hooks, contexts, stores, or abstractions, read
  `spec/code/frontend/cohesion-coupling.md`.
- Before finishing frontend work, check
  `spec/code/frontend/review-checklist.md`.

## Design Rules

- For visual design guidance, read `spec/code/design/design-guide.md`.

## Default Behavior

- Do not read every document by default.
- Before writing frontend code, classify the task type and read the matching
  frontend spec first.
- Start with the document that matches the current task.
- If a change spans multiple concerns, read each relevant focused document.
- If the work reveals another affected area while implementing, pause and read
  the additional document for that area before continuing.
- Keep new rules generic unless a file explicitly describes product-specific
  behavior.

## Development History

- After completing a task, append a short development record to
  `spec/history.md`.
- Each history entry should include the date, a concise summary, and the main
  files or areas changed.
- Keep history entries brief and factual.
