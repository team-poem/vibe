# AGENTS.md

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

## Rust Code Rules

Rust guidance is split under `spec/code/rust/`. These rules apply to any Rust
code in this project, including the Tauri native layer and PoC binaries. Read
the relevant file for the task:

- When writing or editing Rust functions, structs, traits, enums, modules, or
  type signatures, read `spec/code/rust/idioms.md`.
- When designing or changing error types, propagation, or fallible APIs, read
  `spec/code/rust/errors.md`.
- When working with threads, channels, locks, async tasks, or shared state,
  read `spec/code/rust/concurrency.md`.
- Before finishing Rust work, check `spec/code/rust/tooling.md` and run the
  review checklist.

## Design Rules

- For visual design guidance, read `spec/code/design/design-guide.md`.

## Default Behavior

- Do not read every document by default.
- Before writing code, classify the task type and read the matching spec first.
- Start with the document that matches the current task.
- If a change spans multiple concerns, read each relevant focused document.
- If the work reveals another affected area while implementing, pause and read
  the additional document for that area before continuing.
- Keep new rules generic unless a file explicitly describes product-specific
  behavior.

## Spec Reference Disclosure

Before writing or editing code, the agent must announce which spec documents
it is consulting and why. This makes it visible to the user which rules are
shaping the code.

- **Rule name:** Spec Reference Disclosure.
- **When to apply:** Any time the agent is about to write or edit code in this
  repository.
- **Format:** State it in plain prose to the user, then write the code. Example
  shape:

  > 이 작업은 [작업 종류]에 해당하므로 [`spec/code/...`] 의 [관련 섹션]을 참고해서
  > 코드를 작성하겠습니다.

- **What to include:** The specific document path(s), the section(s) being
  applied, and a one-line reason. Multiple documents are listed when the work
  spans concerns.
- **What not to include:** Do not paraphrase the entire document. The
  disclosure is a pointer, not a summary.
- **After coding:** When reporting the result, mention if any rule turned out
  to be in tension with the change so the user can decide whether the rule or
  the code should adjust.

## Language Convention

- All code, identifiers, comments, and error messages are written in English.
- Korean is used only for communication between the user and the agent.

## Development History

- After completing a task, append a development record to `spec/history.md`.
- An entry summarizes the development process, not just a one-line changelog.
  Include the date, the goal, the steps taken, measurements or results, issues
  encountered, decisions made, and any interface contract that carries into the
  next step.
- Keep entries factual but with enough context that a future reader can
  understand why the work was done.
