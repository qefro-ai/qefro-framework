import { ApiError } from "./api";

const MESSAGES: Record<number, string> = {
  401: "Please sign in to continue.",
  403: "You don't have permission to perform this action.",
  404: "That record was not found.",
  409: "This action conflicts with the current state.",
  413: "The file or payload is too large.",
  422: "Please fix the highlighted fields.",
  429: "Too many requests. Try again in a moment.",
};

export function friendlyError(err: unknown): string {
  if (err instanceof ApiError) {
    const mapped = MESSAGES[err.status];
    const message = err.message || mapped || "Something went wrong.";
    if (/select |from |stack|postgres|sqlx|password|secret/i.test(message)) {
      return mapped || "Something went wrong.";
    }
    if (mapped && /^(forbidden|unauthorized|unauthenticated|not_found|not found)$/i.test(message)) {
      return mapped;
    }
    return message;
  }
  if (err instanceof Error) return err.message;
  return "Something went wrong.";
}
