import { z } from 'zod';

/* ── Shared constants ── */
export const PROGRAM_OPTIONS = [
  'BA Malayalam',
  'BA English',
  'BA History',
  'BA Economics',
  'BA Political Science',
  'BA Sociology',
  'BCom',
  'BSc Mathematics',
  'BSc Computer Science',
] as const;

export const SEMESTER_OPTIONS = ['Sem 1', 'Sem 2', 'Sem 3', 'Sem 4', 'Sem 5', 'Sem 6'] as const;

export const LSC_OPTIONS = [
  'LSC Kochi-01',
  'LSC Trivandrum-02',
  'LSC Calicut-03',
  'LSC Thrissur-04',
  'LSC Kollam-05',
  'LSC Kannur-06',
] as const;

/* ── Helpers ── */
const phoneRegex = /^[6-9]\d{9}$/;
const rollRegex = /^\d{6}$/;

/* ── Login ── */
export const loginSchema = z.object({
  identifier: z
    .string()
    .min(1, 'Roll Number or Email is required')
    .refine(
      (v) => v.includes('@') || rollRegex.test(v.trim()) || v.trim().length >= 3,
      'Enter a valid Roll Number or Email',
    ),
  password: z.string().min(1, 'Password is required').min(6, 'Password must be at least 6 characters'),
  rememberMe: z.boolean().optional().default(false),
});
export type LoginInput = z.infer<typeof loginSchema>;

/* ── Signup ──
   Required per spec: Full Name, Roll Number, Phone Number, Email,
   Program, Semester, LSC, Password, Confirm Password, Terms.
   Auto role = STUDENT (no field, no selector).
*/
export const signupSchema = z
  .object({
    fullName: z
      .string()
      .min(1, 'Full name is required')
      .min(2, 'Name is too short')
      .max(80, 'Name is too long')
      .regex(/^[A-Za-z .'-]+$/, 'Only letters, spaces and . \' - allowed'),
    rollNumber: z
      .string()
      .min(1, 'Roll Number is required')
      .regex(rollRegex, 'Roll number must be exactly 6 digits'),
    phoneNumber: z
      .string()
      .min(1, 'Phone number is required')
      .regex(phoneRegex, 'Enter a valid 10-digit Indian mobile number'),
    email: z.string().min(1, 'Email is required').email('Enter a valid email address'),
    program: z.string().min(1, 'Please select your program'),
    semester: z.string().min(1, 'Please select your semester'),
    lsc: z.string().min(1, 'Please select your LSC'),
    password: z
      .string()
      .min(1, 'Password is required')
      .min(8, 'At least 8 characters')
      .regex(/[A-Z]/, 'Include an uppercase letter')
      .regex(/[a-z]/, 'Include a lowercase letter')
      .regex(/\d/, 'Include a number'),
    confirmPassword: z.string().min(1, 'Please confirm your password'),
    terms: z.literal(true, {
      errorMap: () => ({ message: 'You must accept the Terms & Privacy Policy' }),
    }),
  })
  .refine((d) => d.password === d.confirmPassword, {
    path: ['confirmPassword'],
    message: 'Passwords do not match',
  });

export type SignupInput = z.infer<typeof signupSchema>;

/* ── Profile (self-service, editable fields only) ──
   Mirrors the backend `updateStudentProfileSchema`: only Full Name and Phone
   Number are editable. Roll Number, Program, Semester and LSC are
   institution-controlled and are never part of this form.
   The phone pattern is intentionally the backend's broader one
   (`PHONE_REGEX` in validators/student.validator.ts) so client and server
   agree on what is valid.
*/
const profilePhoneRegex = /^\+?[0-9][0-9\s-]{5,18}$/;

export const profileSchema = z.object({
  fullName: z
    .string()
    .min(1, 'Full name is required')
    .trim()
    .min(2, 'Full name must be at least 2 characters')
    .max(100, 'Full name must be at most 100 characters'),
  phoneNumber: z
    .string()
    .min(1, 'Phone number is required')
    .trim()
    .regex(profilePhoneRegex, 'Enter a valid phone number'),
});

export type ProfileInput = z.infer<typeof profileSchema>;

/* ── Change password ──
   Mirrors the backend `changePasswordSchema`: current password is required,
   the new password must meet the strength rules, and both must match. */
export const changePasswordSchema = z
  .object({
    currentPassword: z.string().min(1, 'Current password is required').max(128),
    newPassword: z
      .string()
      .min(1, 'New password is required')
      .min(8, 'At least 8 characters')
      .max(128)
      .regex(/[A-Za-z]/, 'Include a letter')
      .regex(/\d/, 'Include a number'),
    confirmPassword: z.string().min(1, 'Please confirm your new password').max(128),
  })
  .refine((d) => d.newPassword === d.confirmPassword, {
    path: ['confirmPassword'],
    message: 'Passwords do not match',
  })
  .refine((d) => d.newPassword !== d.currentPassword, {
    path: ['newPassword'],
    message: 'New password must be different from your current password',
  });

export type ChangePasswordInput = z.infer<typeof changePasswordSchema>;

/* ── Forgot / Reset ── */
export const forgotPasswordSchema = z.object({
  email: z.string().min(1, 'Email is required').email('Enter a valid email address'),
});
export type ForgotPasswordInput = z.infer<typeof forgotPasswordSchema>;

export const resetPasswordSchema = z
  .object({
    password: z
      .string()
      .min(1, 'Password is required')
      .min(8, 'At least 8 characters')
      .regex(/[A-Z]/, 'Include an uppercase letter')
      .regex(/[a-z]/, 'Include a lowercase letter')
      .regex(/\d/, 'Include a number'),
    confirmPassword: z.string().min(1, 'Please confirm your password'),
  })
  .refine((d) => d.password === d.confirmPassword, {
    path: ['confirmPassword'],
    message: 'Passwords do not match',
  });
export type ResetPasswordInput = z.infer<typeof resetPasswordSchema>;
