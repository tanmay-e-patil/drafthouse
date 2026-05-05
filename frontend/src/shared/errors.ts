import { toast } from "sonner";

export class ApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

export function isInaccessibleDocumentError(error: unknown): boolean {
  return error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 404);
}

export function notifyTransientError(error: unknown): void {
  if (error instanceof ApiError) {
    if (error.status === 429) {
      toast.error("You're doing that too quickly. Please wait a moment and try again.");
      return;
    }

    if (error.status >= 500) {
      toast.error("Something went wrong on our side. Please try again in a moment.");
      return;
    }
  }

  if (error instanceof TypeError) {
    toast.error("Network error. Check your connection and try again.");
  }
}
