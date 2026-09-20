import { useAuthStore } from "./store";
import { refreshApi } from "./api";

/**
 * Single-flight access-token refresh (#17).
 *
 * Every caller shares one in-flight refresh request, so concurrent 401s
 * cannot stampede the refresh endpoint. A failed refresh only clears the
 * access token; callers surface their original error.
 */
let refreshInFlight: Promise<string | null> | null = null;

export function refreshAccessToken(): Promise<string | null> {
  if (!refreshInFlight) {
    refreshInFlight = refreshApi()
      .then((data) => {
        useAuthStore.getState().setAccessToken(data.access_token);
        return data.access_token;
      })
      .catch(() => {
        useAuthStore.setState({ accessToken: null });
        return null;
      })
      .finally(() => {
        refreshInFlight = null;
      });
  }
  return refreshInFlight;
}
