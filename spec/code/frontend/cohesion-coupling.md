# Cohesion And Coupling

Keep related code together while minimizing unnecessary dependencies between
modules and components.

## Considering Form Cohesion

**Rule:** Choose field-level or form-level validation based on the form's
requirements.

**Reasoning:**

- Field-level validation works well for independent fields.
- Form-level validation works well for related or interdependent fields.
- Validation shape should match the business rules.

```tsx
import { useForm } from "react-hook-form";

export default function ProfileForm() {
  const {
    register,
    formState: { errors },
    handleSubmit,
  } = useForm<ProfileFormValues>();

  const onSubmit = handleSubmit((formData) => {
    console.log("Form submitted:", formData);
  });

  return (
    <form onSubmit={onSubmit}>
      <input
        {...register("name", {
          validate: (value) =>
            value.trim() === "" ? "Please enter your name." : true,
        })}
        placeholder="Name"
      />
      {errors.name && <p>{errors.name.message}</p>}

      <button type="submit">Submit</button>
    </form>
  );
}
```

```tsx
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import * as z from "zod";

const profileFormSchema = z.object({
  name: z.string().min(1, "Please enter your name."),
  email: z.string().min(1, "Please enter your email.").email("Invalid email."),
});

export default function ProfileForm() {
  const {
    register,
    formState: { errors },
    handleSubmit,
  } = useForm<ProfileFormValues>({
    resolver: zodResolver(profileFormSchema),
    defaultValues: { name: "", email: "" },
  });

  const onSubmit = handleSubmit((formData) => {
    console.log("Form submitted:", formData);
  });

  return (
    <form onSubmit={onSubmit}>
      <input {...register("name")} placeholder="Name" />
      {errors.name && <p>{errors.name.message}</p>}

      <input {...register("email")} placeholder="Email" />
      {errors.email && <p>{errors.email.message}</p>}

      <button type="submit">Submit</button>
    </form>
  );
}
```

## Organizing Code By Feature Or Domain

**Rule:** Organize directories by feature or domain, not only by file type.

**Reasoning:**

- Keeps related files together.
- Simplifies feature development and deletion.
- Reduces cross-domain coupling.

```text
src/
├── app/
│   └── App.tsx
├── shared/
│   ├── components/
│   ├── hooks/
│   └── utils/
├── domains/
│   ├── user/
│   │   ├── components/
│   │   ├── hooks/
│   │   └── index.ts
│   ├── product/
│   │   ├── components/
│   │   ├── hooks/
│   │   └── index.ts
│   └── order/
│       ├── components/
│       ├── hooks/
│       └── index.ts
└── main.tsx
```

## Relating Constants To Logic

**Rule:** Define constants near the logic they support, or name them so the
relationship is obvious.

```typescript
const TOAST_DISMISS_DELAY_MS = 3000;

async function showSavedToast() {
  toast.success("Saved");
  await delay(TOAST_DISMISS_DELAY_MS);
  toast.dismiss();
}
```

## Avoiding Premature Abstraction

**Rule:** Avoid abstracting duplicated code too early when use cases may
diverge.

**Reasoning:**

- Prevents unrelated features from becoming tied together.
- Keeps each feature easier to evolve.
- Allows real patterns to emerge before creating shared abstractions.

Before abstracting, ask whether the logic is truly identical and likely to stay
identical. If two screens may need different behavior soon, keep the logic local
until the shared shape is clear.

## Scoping State Management

**Rule:** Break broad state management into focused hooks, stores, or contexts.

**Reasoning:**

- Components depend only on the state they need.
- Reduces unnecessary re-renders.
- Makes state ownership clearer.

```typescript
import { useCallback } from "react";
import { NumberParam, useQueryParam } from "use-query-params";

export const useCardIdQueryParam = () => {
  const [cardIdParam, setCardIdParam] = useQueryParam("cardId", NumberParam);

  const setCardId = useCallback(
    (newCardId: number | undefined) => {
      setCardIdParam(newCardId, "replaceIn");
    },
    [setCardIdParam]
  );

  return [cardIdParam ?? undefined, setCardId] as const;
};
```

## Completion Step

Before finishing frontend work, read `spec/code/frontend/review-checklist.md`
and verify the change against the checklist.
