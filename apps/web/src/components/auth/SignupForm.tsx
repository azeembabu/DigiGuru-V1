"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";

import { ApiError, apiFetch } from "@/lib/api";
import {
  compact,
  tenDigitMobile,
  validateConfirmPassword,
  validateEmail,
  validateFullName,
  validatePassword,
  validatePhone,
  validateRequiredSelection,
  validateRollNumber,
  type FieldErrors,
} from "@/lib/validation";
import { loadAcademicReference, loadSemesters, type ReferenceOption } from "@/lib/reference";
import { Field } from "@/components/ui/Field";
import { FormError } from "@/components/auth/FormError";
import { PasswordInput } from "@/components/auth/PasswordInput";
import { SelectInput } from "@/components/ui/SelectInput";
import { SubmitButton } from "@/components/ui/SubmitButton";
import { TextInput } from "@/components/ui/TextInput";

type SignupResponse = { user_id: string; student_id: string };

export function SignupForm() {
  const router = useRouter();

  const [programs, setPrograms] = useState<ReferenceOption[]>([]);
  const [lscs, setLscs] = useState<ReferenceOption[]>([]);
  const [semesters, setSemesters] = useState<ReferenceOption[]>([]);
  const [referenceError, setReferenceError] = useState<string | null>(null);

  const [programId, setProgramId] = useState("");
  const [errors, setErrors] = useState<FieldErrors>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    let cancelled = false;
    loadAcademicReference()
      .then((reference) => {
        if (cancelled) return;
        setPrograms(reference.programs);
        setLscs(reference.lscs);
        setReferenceError(null);
      })
      .catch(() => {
        if (cancelled) return;
        setReferenceError(
          "Could not load the list of programs and study centres. Please refresh the page to try again.",
        );
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Semesters belong to a program, so they reload whenever the program
  // changes. The previous program's semesters are cleared by the change
  // handler rather than here, so this effect never sets state synchronously.
  useEffect(() => {
    if (!programId) return;
    let cancelled = false;
    loadSemesters(programId)
      .then((rows) => {
        if (!cancelled) setSemesters(rows);
      })
      .catch(() => {
        if (!cancelled) setSemesters([]);
      });
    return () => {
      cancelled = true;
    };
  }, [programId]);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;

    const data = new FormData(event.currentTarget);
    const value = (key: string) => String(data.get(key) ?? "").trim();

    const full_name = value("full_name");
    const roll_number = value("roll_number").toUpperCase();
    const phone_raw = value("phone_number");
    const email = value("email");
    const program_id = value("program_id");
    const semester_id = value("semester_id");
    const lsc_id = value("lsc_id");
    const password = String(data.get("password") ?? "");
    const confirm_password = String(data.get("confirm_password") ?? "");

    const clientErrors = compact({
      full_name: validateFullName(full_name),
      roll_number: validateRollNumber(roll_number),
      phone_number: validatePhone(phone_raw),
      email: validateEmail(email),
      program_id: validateRequiredSelection(program_id, "program"),
      semester_id: validateRequiredSelection(semester_id, "semester"),
      lsc_id: validateRequiredSelection(lsc_id, "study centre"),
      password: validatePassword(password),
      confirm_password: validateConfirmPassword(password, confirm_password),
    });

    setErrors(clientErrors);
    setFormError(null);
    if (Object.keys(clientErrors).length > 0) return;

    setPending(true);
    try {
      await apiFetch<SignupResponse>("/auth/signup", {
        method: "POST",
        body: JSON.stringify({
          full_name,
          roll_number,
          // Send the bare 10 digits; the gateway normalises to E.164 itself.
          phone_number: tenDigitMobile(phone_raw) ?? phone_raw,
          email,
          password,
          program_id,
          semester_id,
          lsc_id,
        }),
      });

      // Signup returns ids only and sets no cookies
      // (`apps/gateway/src/auth/signup.rs`), so the student is not logged in
      // yet — send them to sign in rather than pretending they have a session.
      router.push("/login?registered=1");
    } catch (error) {
      if (error instanceof ApiError) {
        setFormError(error.isValidation ? "Please correct the highlighted fields." : error.message);
        setErrors(error.fieldErrors());
      } else {
        setFormError("Something went wrong. Please try again.");
      }
      setPending(false);
    }
  }

  return (
    // Paired into two columns from `sm` up so the whole form fits a desktop
    // viewport without the page scrolling; it stacks to one column on phones.
    <form onSubmit={onSubmit} noValidate className="space-y-3.5">
      <FormError message={formError ?? referenceError} />

      <div className="grid gap-3.5 sm:grid-cols-2">
        <Field id="full_name" label="Full name" error={errors.full_name}>
          <TextInput
            id="full_name"
            autoComplete="name"
            placeholder="Anjali Menon"
            required
            hasError={Boolean(errors.full_name)}
          />
        </Field>

        <Field
          id="roll_number"
          label="Roll number"
          error={errors.roll_number}
          hint="As printed on your records — e.g. 25XHBML11450."
        >
          <TextInput
            id="roll_number"
            autoComplete="off"
            autoCapitalize="characters"
            placeholder="25XHBML11450"
            required
            hasError={Boolean(errors.roll_number)}
            hasHint
            className="uppercase"
          />
        </Field>

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

        <Field id="phone_number" label="Mobile number" error={errors.phone_number}>
          <TextInput
            id="phone_number"
            type="tel"
            inputMode="numeric"
            autoComplete="tel"
            placeholder="98765 43210"
            required
            hasError={Boolean(errors.phone_number)}
          />
        </Field>

        <Field id="program_id" label="Program" error={errors.program_id}>
          <SelectInput
            id="program_id"
            options={programs}
            placeholder="Select your program"
            value={programId}
            onValueChange={(next) => {
              setProgramId(next);
              // Drop the old program's semesters immediately, so a stale
              // option can never sit selected under a different program.
              setSemesters([]);
            }}
            hasError={Boolean(errors.program_id)}
          />
        </Field>

        <Field id="semester_id" label="Semester" error={errors.semester_id}>
          <SelectInput
            id="semester_id"
            options={semesters}
            placeholder={programId ? "Select your semester" : "Choose a program first"}
            disabled={!programId || semesters.length === 0}
            hasError={Boolean(errors.semester_id)}
            className="disabled:cursor-not-allowed disabled:opacity-60"
          />
        </Field>

        <Field id="lsc_id" label="Study centre (LSC)" error={errors.lsc_id}>
          <SelectInput
            id="lsc_id"
            options={lscs}
            placeholder="Select your study centre"
            hasError={Boolean(errors.lsc_id)}
          />
        </Field>

        <Field id="password" label="Password" error={errors.password} hint="At least 8 characters.">
          <PasswordInput
            id="password"
            autoComplete="new-password"
            hasError={Boolean(errors.password)}
            hasHint
          />
        </Field>

        <Field id="confirm_password" label="Confirm password" error={errors.confirm_password}>
          <PasswordInput
            id="confirm_password"
            autoComplete="new-password"
            hasError={Boolean(errors.confirm_password)}
          />
        </Field>
      </div>

      <SubmitButton pending={pending} pendingLabel="Creating account…">
        Create account
      </SubmitButton>
    </form>
  );
}
