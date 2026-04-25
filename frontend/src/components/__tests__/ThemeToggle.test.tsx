import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const setTheme = vi.fn();
let theme = "system";

vi.mock("next-themes", () => ({
  useTheme: () => ({ theme, setTheme, resolvedTheme: "dark" }),
}));

describe("ThemeToggle", () => {
  it("cycles explicit theme preference through light, dark, and system", async () => {
    const { default: ThemeToggle } = await import("../ThemeToggle");
    const user = userEvent.setup();

    const { rerender } = render(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: /current: system/i }));
    expect(setTheme).toHaveBeenLastCalledWith("light");

    theme = "light";
    rerender(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: /current: light/i }));
    expect(setTheme).toHaveBeenLastCalledWith("dark");

    theme = "dark";
    rerender(<ThemeToggle />);
    await user.click(screen.getByRole("button", { name: /current: dark/i }));
    expect(setTheme).toHaveBeenLastCalledWith("system");
  });
});
