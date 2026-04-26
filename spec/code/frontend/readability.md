# Readability

Improving the clarity and ease of understanding code.

## Naming Magic Numbers

**Rule:** Replace magic numbers with named constants for clarity.

**Reasoning:**

- Improves clarity by giving semantic meaning to unexplained values.
- Enhances maintainability.
- Prevents logic and related values from drifting apart.

```typescript
const ANIMATION_DELAY_MS = 300;

async function onLikeClick() {
  await postLike(url);
  await delay(ANIMATION_DELAY_MS);
  await refetchPostLike();
}
```

## Naming Complex Conditions

**Rule:** Assign complex boolean conditions to named variables.

**Reasoning:**

- Makes the meaning of the condition explicit.
- Reduces cognitive load when scanning filters, guards, and render branches.
- Gives important domain rules a name.

```typescript
const matchedProducts = products.filter((product) => {
  const isSameCategory = product.categories.some(
    (category) => category.id === targetCategory.id
  );

  const isPriceInRange = product.prices.some(
    (price) => price >= minPrice && price <= maxPrice
  );

  return isSameCategory && isPriceInRange;
});
```

Name conditions when the logic is complex, reused, or meaningful to the domain.
Avoid naming very simple, single-use conditions.

## Simplifying Complex Ternary Operators

**Rule:** Replace complex or nested ternaries with `if` statements, early
returns, or IIFEs.

**Reasoning:**

- Makes conditional logic easier to follow.
- Reduces visual noise in JSX.
- Keeps render branches predictable.

```typescript
const status = (() => {
  if (hasEmailError && hasPasswordError) return "INVALID_FORM";
  if (hasEmailError) return "INVALID_EMAIL";
  if (hasPasswordError) return "INVALID_PASSWORD";
  return "VALID";
})();
```

## Reducing Eye Movement

**Rule:** Colocate simple, localized logic near the JSX or function that uses
it.

**Reasoning:**

- Allows top-to-bottom reading.
- Reduces context switching.
- Keeps small policies visible without over-abstracting.

```tsx
type Role = "admin" | "viewer";

const RoleActions = ({ role }: { role: Role }) => {
  switch (role) {
    case "admin":
      return (
        <div>
          <Button disabled={false}>Invite</Button>
          <Button disabled={false}>View</Button>
        </div>
      );
    case "viewer":
      return (
        <div>
          <Button disabled={true}>Invite</Button>
          <Button disabled={false}>View</Button>
        </div>
      );
  }
};
```

```tsx
const UserActions = ({ role }: { role: "admin" | "viewer" }) => {
  const policy = {
    admin: { canInvite: true, canView: true },
    viewer: { canInvite: false, canView: true },
  }[role];

  return (
    <div>
      <Button disabled={!policy.canInvite}>Invite</Button>
      <Button disabled={!policy.canView}>View</Button>
    </div>
  );
};
```

## Completion Step

Before finishing frontend work, read `spec/code/frontend/review-checklist.md`
and verify the change against the checklist.
