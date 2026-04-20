import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import { useDebounce } from "../useDebounce";

function TestComponent({
  callback,
  delay,
  onDebounce,
}: {
  callback: (value: string) => void;
  delay: number;
  onDebounce: (fn: (value: string) => void) => void;
}) {
  const debounced = useDebounce(callback, delay);
  onDebounce(debounced);
  return null;
}

describe("useDebounce", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("delays callback execution by the specified delay", () => {
    const callback = vi.fn();
    let triggerDebounce: (value: string) => void = () => {};

    render(
      <TestComponent
        callback={callback}
        delay={500}
        onDebounce={(fn) => {
          triggerDebounce = fn;
        }}
      />
    );

    act(() => {
      triggerDebounce("hello");
    });

    expect(callback).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith("hello");
  });

  it("resets timer on rapid calls, only invoking with last value", () => {
    const callback = vi.fn();
    let triggerDebounce: (value: string) => void = () => {};

    render(
      <TestComponent
        callback={callback}
        delay={500}
        onDebounce={(fn) => {
          triggerDebounce = fn;
        }}
      />
    );

    act(() => {
      triggerDebounce("first");
      triggerDebounce("second");
      triggerDebounce("third");
    });

    expect(callback).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith("third");
  });

  it("does not invoke callback before delay elapses", () => {
    const callback = vi.fn();
    let triggerDebounce: (value: string) => void = () => {};

    render(
      <TestComponent
        callback={callback}
        delay={1000}
        onDebounce={(fn) => {
          triggerDebounce = fn;
        }}
      />
    );

    act(() => {
      triggerDebounce("hello");
      vi.advanceTimersByTime(499);
    });

    expect(callback).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(501);
    });

    expect(callback).toHaveBeenCalledTimes(1);
  });

  it("cleans up timer on unmount", () => {
    const callback = vi.fn();
    let triggerDebounce: (value: string) => void = () => {};

    const { unmount } = render(
      <TestComponent
        callback={callback}
        delay={500}
        onDebounce={(fn) => {
          triggerDebounce = fn;
        }}
      />
    );

    act(() => {
      triggerDebounce("hello");
    });

    unmount();

    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(callback).not.toHaveBeenCalled();
  });
});
