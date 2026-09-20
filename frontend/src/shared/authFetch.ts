import { useAuthStore } from "#/features/auth/store";
import { refreshAccessToken } from "#/features/auth/tokenRefresh";

/**
 * Authenticated fetch with single-flight token refresh and one retry (#17).
 *
 * A 401 on an authenticated request triggers the shared refresh and retries
 * the original request exactly once with the new token. Any other status —
 * or a failed refresh — returns the response/error unchanged so callers
 * surface it instead of looping.
 */
export async function authFetch(url: string, init: RequestInit = {}): Promise<Response> {
  const token = useAuthStore.getState().accessToken;
  const headers = authHeaders(init.headers, token);
  const res = await fetch(url, { ...init, headers, credentials: "include" });

  if (res.status !== 401 || !token) {
    return res;
  }

  const refreshed = await refreshAccessToken();
  if (!refreshed) {
    return res;
  }
  return fetch(url, {
    ...init,
    headers: authHeaders(init.headers, refreshed),
    credentials: "include",
  });
}

function authHeaders(base: HeadersInit | undefined, token: string): Record<string, string> {
  return {
    "Content-Type": "application/json",
    ...(base as Record<string, string> | undefined),
    Authorization: `Bearer ${token}`,
  };
}
