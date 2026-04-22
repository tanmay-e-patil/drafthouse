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

export async function getMeApi(accessToken: string): Promise<MeResponse> {
  const res = await fetch(`${API_BASE}/auth/me`, {
    headers: { Authorization: `Bearer ${accessToken}` },
    credentials: "include",
  });
  if (!res.ok) throw new Error("Failed to fetch profile");
  return res.json() as Promise<MeResponse>;
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
