# Styling

Keep styling consistent and easy to maintain. Prefer project-level styling
patterns over one-off JSX styles.

## Avoid Inline Styles

**Rule:** Do not use JSX inline styles such as `style={{ ... }}` in production
code. Use class-based styling, CSS Modules, Tailwind, Vanilla Extract, or the
project's design system components instead.

**Reasoning:**

- Keeps visual rules reusable and searchable.
- Prevents design values from being scattered across JSX.
- Makes responsive, pseudo-state, and theme styling easier to manage.
- Encourages consistent use of design tokens and shared components.

```tsx
// Avoid
const Toolbar = () => {
  return <div style={{ display: "flex", gap: "8px" }}>...</div>;
};

// Prefer
const Toolbar = () => {
  return <div className="toolbar">...</div>;
};
```

## Allowed Exceptions

Inline styles are allowed only when there is a clear technical reason.

- Dynamic CSS variables for values that must be computed at runtime.
- Third-party library APIs that require inline style objects.
- Temporary prototypes, stories, or tests where styling is not part of the
  production surface.

```tsx
const ProgressBar = ({ progress }: { progress: number }) => {
  return (
    <div
      className="progressBar"
      style={{ "--progress": progress } as React.CSSProperties}
    />
  );
};
```

## Completion Step

Before finishing frontend work, read `spec/code/frontend/review-checklist.md`
and verify the change against the checklist.
