# Function And Component Style

Use different declaration styles based on the role of the code. This keeps files
predictable and makes intent visible at a glance.

## Top-Level App And Page Components

**Rule:** Use `export default function` for top-level app, page, route, and
screen components.

**Reasoning:**

- Makes entry components easy to find.
- Keeps route/page files consistent.
- Works well with frameworks that expect default exports for pages.

```tsx
export default function SettingsPage() {
  return (
    <main>
      <SettingsHeader />
      <SettingsForm />
    </main>
  );
}
```

## Local Components And Basic Functions

**Rule:** Use `const` with arrow functions for local components and ordinary
synchronous helper functions.

**Reasoning:**

- Keeps implementation details visually lightweight.
- Makes local helpers easy to colocate with the component that uses them.
- Avoids mixing multiple function styles without meaning.

```tsx
const SettingsHeader = () => {
  return <h1>Settings</h1>;
};

const formatUserName = (firstName: string, lastName: string) => {
  return `${firstName} ${lastName}`;
};
```

## Async Functions

**Rule:** Use `async function` for asynchronous operations, especially API
calls, event flows with multiple awaits, and functions exported from modules.

**Reasoning:**

- Makes asynchronous behavior obvious from the declaration.
- Improves stack traces and readability.
- Keeps side-effectful flows distinct from simple local helpers.

```typescript
async function fetchUser(userId: string): Promise<User> {
  const response = await http.get<User>(`/users/${userId}`);
  return response.data;
}

async function handleSaveClick() {
  await saveSettings();
  await refetchSettings();
}
```

## Event Handlers

**Rule:** Keep simple event handlers inline or as local arrow functions. Use
`async function` when the handler contains a meaningful async flow.

```tsx
const ToggleButton = () => {
  const [enabled, setEnabled] = useState(false);

  const handleClick = () => {
    setEnabled((current) => !current);
  };

  return <button onClick={handleClick}>{enabled ? "On" : "Off"}</button>;
};
```

```tsx
export default function SaveButton() {
  async function handleClick() {
    await saveCurrentDraft();
    await showSavedToast();
  }

  return <button onClick={handleClick}>Save</button>;
}
```

## Completion Step

Before finishing frontend work, read `spec/code/frontend/review-checklist.md`
and verify the change against the checklist.
