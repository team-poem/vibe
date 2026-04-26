# Frontend Review Checklist

Use this checklist before submitting frontend code.

- Top-level app, page, route, and screen components use `export default function`.
- Local components and synchronous helpers use `const` arrow functions.
- Async flows use `async function`.
- Inline styles are avoided in production JSX.
- Magic numbers have meaningful names.
- Complex conditions are named when they carry domain meaning.
- Complex or nested ternaries are replaced with clearer conditional logic.
- Significantly different render branches are separated into components.
- Complex interactions are extracted into dedicated components, hooks, or guards.
- Hooks and similar functions return consistent shapes.
- Functions do not hide unrelated side effects.
- Names reveal special behavior such as auth, logging, caching, or persistence.
- Related code is colocated by feature or domain.
- Shared abstractions are introduced only when the use cases are likely to stay aligned.
- State is scoped to the smallest practical owner.

## Completion Step

This checklist is the required final review step before finishing frontend work.
