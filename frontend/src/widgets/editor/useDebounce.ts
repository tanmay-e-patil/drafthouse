import { useEffect, useRef, useCallback } from "react";

export function useDebounce<T>(callback: (value: T) => void, delay: number): (value: T) => void {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef(callback);

  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current);
      }
    };
  }, []);

  const debouncedFn = useCallback(
    (value: T) => {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current);
      }
      timerRef.current = setTimeout(() => {
        callbackRef.current(value);
      }, delay);
    },
    [delay]
  );

  return debouncedFn;
}
