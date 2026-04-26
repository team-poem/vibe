# Predictability

Ensuring code behaves as expected based on its name, parameters, and context.

## Standardizing Return Types

**Rule:** Use consistent return types for similar functions and hooks.

**Reasoning:**

- Developers can predict how similar APIs are consumed.
- Reduces confusion from inconsistent return shapes.
- Makes refactoring safer.

```typescript
import { useQuery, type UseQueryResult } from "@tanstack/react-query";

export const useUser = (): UseQueryResult<User, Error> => {
  return useQuery({ queryKey: ["user"], queryFn: fetchUser });
};

export const useServerTime = (): UseQueryResult<Date, Error> => {
  return useQuery({
    queryKey: ["serverTime"],
    queryFn: fetchServerTime,
  });
};
```

## Validation Result Shape

**Rule:** Use consistent validation return types, preferably discriminated
unions.

```typescript
type ValidationResult = { ok: true } | { ok: false; reason: string };

const checkIsNameValid = (name: string): ValidationResult => {
  if (name.length === 0) {
    return { ok: false, reason: "Name cannot be empty." };
  }

  if (name.length >= 20) {
    return { ok: false, reason: "Name cannot be longer than 20 characters." };
  }

  return { ok: true };
};

const checkIsAgeValid = (age: number): ValidationResult => {
  if (!Number.isInteger(age)) {
    return { ok: false, reason: "Age must be an integer." };
  }

  if (age < 18) {
    return { ok: false, reason: "Age must be 18 or older." };
  }

  if (age > 99) {
    return { ok: false, reason: "Age must be 99 or younger." };
  }

  return { ok: true };
};
```

## Revealing Hidden Logic

**Rule:** Avoid hidden side effects. Functions should only perform actions
implied by their name and signature.

**Reasoning:**

- Makes behavior predictable.
- Prevents surprising side effects.
- Creates more testable code.

```typescript
async function fetchBalance(): Promise<number> {
  const balance = await http.get<number>("/balance");
  return balance;
}

async function handleUpdateClick() {
  const balance = await fetchBalance();

  logging.log("balance_fetched");
  await syncBalance(balance);
}
```

## Unique And Descriptive Names

**Rule:** Use unique, descriptive names for wrappers, services, and functions.

**Reasoning:**

- Avoids ambiguity.
- Makes special behavior visible at the call site.
- Helps readers understand what kind of side effects may happen.

```typescript
import { http as httpLibrary } from "@some-library/http";

export const httpService = {
  async getWithAuth(url: string) {
    const token = await fetchToken();

    return httpLibrary.get(url, {
      headers: { Authorization: `Bearer ${token}` },
    });
  },
};

async function fetchUser() {
  return await httpService.getWithAuth("/user");
}
```

## Completion Step

Before finishing frontend work, read `spec/code/frontend/review-checklist.md`
and verify the change against the checklist.
