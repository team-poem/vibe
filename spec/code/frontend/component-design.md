# Component Design

Keep components focused, readable, and explicit about the responsibility they
own.

## Abstracting Implementation Details

**Rule:** Abstract complex logic or interactions into dedicated components,
hooks, or guards.

**Reasoning:**

- Reduces cognitive load by separating concerns.
- Improves testability and maintainability.
- Keeps page components focused on composition.

```tsx
export default function LoginPage() {
  return (
    <AuthGuard>
      <LoginStartPage />
    </AuthGuard>
  );
}

const AuthGuard = ({ children }: { children: ReactNode }) => {
  const status = useCheckLoginStatus();

  useEffect(() => {
    if (status === "LOGGED_IN") {
      location.href = "/home";
    }
  }, [status]);

  return status !== "LOGGED_IN" ? children : null;
};

const LoginStartPage = () => {
  return <LoginForm />;
};
```

## Dedicated Interaction Components

**Rule:** Move multi-step user interactions into focused components when the
interaction would otherwise make a page difficult to scan.

```tsx
export default function FriendInvitation() {
  const { data } = useFriendQuery();

  return (
    <>
      <InviteButton name={data.name} />
      <FriendProfile friend={data} />
    </>
  );
}

const InviteButton = ({ name }: { name: string }) => {
  async function handleClick() {
    const canInvite = await overlay.openAsync(({ isOpen, close }) => (
      <ConfirmDialog
        isOpen={isOpen}
        onClose={close}
        title={`Share with ${name}`}
      />
    ));

    if (canInvite) {
      await sendPush();
    }
  }

  return <Button onClick={handleClick}>Invite</Button>;
};
```

## Separating Conditional Rendering Paths

**Rule:** Separate significantly different UI or logic into distinct
components.

**Reasoning:**

- Avoids large conditional blocks inside one component.
- Gives each specialized component a single responsibility.
- Makes role, state, or mode-specific behavior easier to change.

```tsx
const SubmitButton = () => {
  const isViewer = useRole() === "viewer";

  return isViewer ? <ViewerSubmitButton /> : <AdminSubmitButton />;
};

const ViewerSubmitButton = () => {
  return <TextButton disabled>Submit</TextButton>;
};

const AdminSubmitButton = () => {
  useEffect(() => {
    showAnimation();
  }, []);

  return <Button type="submit">Submit</Button>;
};
```

## Eliminating Props Drilling With Composition

**Rule:** Prefer component composition over passing props through intermediate
components that do not use them.

**Reasoning:**

- Reduces coupling between unrelated components.
- Makes refactoring easier.
- Keeps data flow closer to where data is actually used.

```tsx
export default function ItemEditModal({
  open,
  items,
  recommendedItems,
  onConfirm,
  onClose,
}: ItemEditModalProps) {
  const [keyword, setKeyword] = useState("");

  return (
    <Modal open={open} onClose={onClose}>
      <div className="toolbar">
        <Input
          value={keyword}
          onChange={(event) => setKeyword(event.target.value)}
          placeholder="Search items..."
        />
        <Button onClick={onClose}>Close</Button>
      </div>

      <ItemEditList
        keyword={keyword}
        items={items}
        recommendedItems={recommendedItems}
        onConfirm={onConfirm}
      />
    </Modal>
  );
}
```

## Completion Step

Before finishing frontend work, read `spec/code/frontend/review-checklist.md`
and verify the change against the checklist.
