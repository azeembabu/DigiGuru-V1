"use client";

import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";

import { ApiError, apiFetch } from "@/lib/api";
import { compact, validateEmail, type FieldErrors } from "@/lib/validation";
import { Field } from "@/components/ui/Field";
import { FormError } from "@/components/auth/FormError";
import { PasswordInput } from "@/components/auth/PasswordInput";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { TextInput } from "@/components/ui/TextInput";

// `POST /api/v1/auth/login` takes email + password only
// (`apps/gateway/src/auth/login.rs`). It sets the access/refresh cookies
// itself, so there is no token for this component to store.
type LoginResponse = {
  user_id: string;
  role: string;
  is_first_login: boolean | null;
};

export function LoginForm() {
  const router = useRouter();
  const [errors, setErrors] = useState<FieldErrors>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;

    const data = new FormData(event.currentTarget);
    const email = String(data.get("email") ?? "").trim();
    const password = String(data.get("password") ?? "");

    // Only the email is shape-checked here. The password is deliberately not
    // length-validated on the login form: telling someone their password is
    // "too short" before submitting leaks the rule to whoever is guessing.
    const clientErrors = compact({
      email: validateEmail(email),
      password: password ? null : "Password is required.",
    });

    setErrors(clientErrors);
    setFormError(null);
    if (Object.keys(clientErrors).length > 0) return;

    setPending(true);
    try {
      const result = await apiFetch<LoginResponse>("/auth/login", {
        method: "POST",
        body: JSON.stringify({ email, password }),
      });

      // NN-2: the greeting is driven by `is_first_login`, which the gateway
      // owns and flips server-side. The client only forwards where to land.
      router.push(result.is_first_login ? "/classroom?welcome=1" : "/dashboard");
      router.refresh();
    } catch (error) {
      if (error instanceof ApiError) {
        // The gateway returns one identical error for "no such email" and
        // "wrong password" on purpose — keep it undifferentiated here too.
        setFormError(
          error.code === "UNAUTHORIZED"
            ? "That email and password combination is not recognised."
            : error.message,
        );
        setErrors(error.fieldErrors());
      } else {
        setFormError("Something went wrong. Please try again.");
      }
      setPending(false);
    }
  }

  return (
    <form onSubmit={onSubmit} noValidate className="space-y-5">
      <FormError message={formError} />

      <Field id="email" label="Email" error={errors.email}>
        <TextInput
          id="email"
          type="email"
          inputMode="email"
          autoComplete="email"
          placeholder="you@example.com"
          required
          hasError={Boolean(errors.email)}
        />
      </Field>

      <Field id="password" label="Password" error={errors.password}>
        <PasswordInput id="password" autoComplete="current-password" hasError={Boolean(errors.password)} />
      </Field>

      {/*
        No "Forgot your password?" link yet. The gateway already exposes
        `POST /api/v1/auth/forgot-password` and `/auth/reset-password`
        (`apps/gateway/src/auth/mod.rs`), but the /forgot-password route does
        not exist on the frontend — linking to it would 404. It lands with
        that page rather than as a dead link here.
      */}

      <SubmitButton pending={pending} pendingLabel="Signing in…">
        Sign in
      </SubmitButton>
    </form>
  );
}
