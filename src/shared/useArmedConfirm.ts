import { useEffect, useState } from "react";

const DISARM_AFTER_MS = 3000;

/// Two-step destructive action guard: the first trigger arms the control
/// (caller renders a "really?" state), the second within the window fires
/// the action. Auto-disarms after a few seconds.
export const useArmedConfirm = (onConfirm: () => void) => {
  const [armed, setArmed] = useState(false);

  useEffect(() => {
    if (!armed) {
      return;
    }
    const timer = window.setTimeout(() => setArmed(false), DISARM_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, [armed]);

  const trigger = () => {
    if (armed) {
      setArmed(false);
      onConfirm();
    } else {
      setArmed(true);
    }
  };

  return { armed, trigger };
};
