import { useEffect, useRef, useCallback } from "react";

export function useDebounce<T>(callback: (value: T) => void, delay: number): (value: T) => void {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef(callback);

  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  const cancel = useCallback(() => {
    const timer = timerRef.current;
    if (timer !== null) {
      clearTimeout(timer);
      timerRef.current = null;
    }
  }, []);

  useEffect(() => cancel, [cancel]);

  const debouncedFn = useCallback(
    (value: T) => {
      cancel();
      timerRef.current = setTimeout(() => {
        callbackRef.current(value);
      }, delay);
    },
    [cancel, delay]
  );

  return debouncedFn;
}
