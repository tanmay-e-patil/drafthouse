import { beforeEach, describe, expect, it, vi } from "vitest";
import { toast } from "sonner";
import { ApiError, isInaccessibleDocumentError, notifyTransientError } from "../errors";

vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));
beforeEach(() => vi.clearAllMocks());

describe("error feedback", () => {
  it.each([500, 502, 503])("uses a generic message for HTTP %i", (status) => {
    notifyTransientError(new ApiError("private server detail", status));
    expect(toast.error).toHaveBeenCalledExactlyOnceWith(
      "Something went wrong on our side. Please try again in a moment.",
    );
  });

  it("explains rate limiting", () => {
    notifyTransientError(new ApiError("rate limited", 429));
    expect(toast.error).toHaveBeenCalledExactlyOnceWith(
      "You're doing that too quickly. Please wait a moment and try again.",
    );
  });

  it("explains network failures", () => {
    notifyTransientError(new TypeError("Failed to fetch"));
    expect(toast.error).toHaveBeenCalledExactlyOnceWith(
      "Network error. Check your connection and try again.",
    );
  });

  it("leaves validation errors to the caller", () => {
    notifyTransientError(new ApiError("Invalid title", 400));
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("uses the caller's fallback for other errors", () => {
    notifyTransientError(new ApiError("Forbidden", 403), "Failed to update role");
    expect(toast.error).toHaveBeenCalledExactlyOnceWith("Failed to update role");
  });

  it.each([401, 403, 404])("recognizes inaccessible documents with HTTP %i", (status) => {
    expect(isInaccessibleDocumentError(new ApiError("Unavailable", status))).toBe(true);
  });

  it("does not classify transient failures as inaccessible documents", () => {
    expect(isInaccessibleDocumentError(new ApiError("Unavailable", 503))).toBe(false);
    expect(isInaccessibleDocumentError(new TypeError("Failed to fetch"))).toBe(false);
  });
});
