const API_BASE = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

export interface LoginResponse {
  access_token: string;
  token_type: string;
  welcome_doc_id?: string | null;
}

export interface ApiError {
  detail: string;
}

export async function loginApi(
  email: string,
  password: string
): Promise<LoginResponse> {
  const res = await fetch(`${API_BASE}/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "include",
    body: JSON.stringify({ email, password }),
  });

  const data = await res.json();

  if (!res.ok) {
    const err = data as ApiError;
    throw new Error(err.detail ?? "Login failed");
  }

  return data as LoginResponse;
}

export async function refreshApi(): Promise<LoginResponse> {
  const res = await fetch(`${API_BASE}/auth/refresh`, {
    method: "POST",
    credentials: "include",
  });

  if (!res.ok) {
    throw new Error("Session expired");
  }

  return res.json() as Promise<LoginResponse>;
}

export interface MeResponse {
  id: string;
  email: string;
  email_verified_at: string | null;
  created_at: string;
}

export interface ChangePasswordResponse {
  message: string;
}

export interface DeleteAccountResponse {
  message: string;
}

export interface ExportResponse {
  message: string;
}

function getAuthHeaders(accessToken: string): Record<string, string> {
  return {
    "Content-Type": "application/json",
    Authorization: `Bearer ${accessToken}`,
  };
}

async function handleAuthedResponse<T>(
  res: Response,
  fallbackError: string
): Promise<T> {
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    const err = data as ApiError;
    throw new Error(err.detail ?? fallbackError);
  }
  return data as T;
}

export async function getMeApi(accessToken: string): Promise<MeResponse> {
  const res = await fetch(`${API_BASE}/auth/me`, {
    headers: { Authorization: `Bearer ${accessToken}` },
    credentials: "include",
  });
  return handleAuthedResponse<MeResponse>(res, "Failed to fetch profile");
}

export async function changePasswordApi(
  accessToken: string,
  currentPassword: string,
  newPassword: string
): Promise<ChangePasswordResponse> {
  const res = await fetch(`${API_BASE}/auth/me/password`, {
    method: "POST",
    headers: getAuthHeaders(accessToken),
    credentials: "include",
    body: JSON.stringify({
      current_password: currentPassword,
      new_password: newPassword,
    }),
  });

  return handleAuthedResponse<ChangePasswordResponse>(
    res,
    "Failed to update password"
  );
}

export async function deleteAccountApi(
  accessToken: string,
  currentPassword: string
): Promise<DeleteAccountResponse> {
  const res = await fetch(`${API_BASE}/auth/me`, {
    method: "DELETE",
    headers: getAuthHeaders(accessToken),
    credentials: "include",
    body: JSON.stringify({ current_password: currentPassword }),
  });

  return handleAuthedResponse<DeleteAccountResponse>(
    res,
    "Failed to delete account"
  );
}

export async function exportAccountDataApi(
  accessToken: string
): Promise<ExportResponse> {
  const res = await fetch(`${API_BASE}/auth/me/export`, {
    method: "POST",
    headers: getAuthHeaders(accessToken),
    credentials: "include",
  });

  return handleAuthedResponse<ExportResponse>(res, "Failed to export account data");
}

export async function logoutApi(): Promise<void> {
  await fetch(`${API_BASE}/auth/logout`, {
    method: "POST",
    credentials: "include",
  });
}

export async function forgotPasswordApi(email: string): Promise<{ message: string }> {
  const res = await fetch(`${API_BASE}/auth/forgot-password`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email }),
  });

  const data = await res.json();

  if (!res.ok) {
    const err = data as ApiError;
    throw new Error(err.detail ?? "Failed to send reset email");
  }

  return data as { message: string };
}

export async function resetPasswordApi(
  token: string,
  newPassword: string
): Promise<{ message: string }> {
  const res = await fetch(`${API_BASE}/auth/reset-password`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ token, new_password: newPassword }),
  });

  const data = await res.json();

  if (!res.ok) {
    const err = data as ApiError;
    throw new Error(err.detail ?? "Failed to reset password");
  }

  return data as { message: string };
}
