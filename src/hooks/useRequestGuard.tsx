import { useRef } from "react";

/**
 * Guards async results against a newer request or session.
 *
 * Returns a monotonically increasing token: call `next()` before starting
 * work, `isCurrent(id)` before applying its result, and `invalidate()` when
 * the surrounding session is reset.
 */
export function useRequestGuard() {
  const current = useRef(0);

  const next = () => {
    current.current += 1;
    return current.current;
  };

  const invalidate = () => {
    current.current += 1;
  };

  const isCurrent = (id: number) => current.current === id;

  return { next, invalidate, isCurrent };
}
