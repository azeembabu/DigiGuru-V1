import { z } from 'zod';

export const studentSignupSchema = z.object({
  full_name: z.string().min(2, 'Full name must be at least 2 characters').max(100),
  email: z.string().email('Invalid email address').max(255).toLowerCase(),
  password: z
    .string()
    .min(8, 'Password must be at least 8 characters')
    .max(128)
    .regex(/[A-Za-z]/, 'Password must contain a letter')
    .regex(/[0-9]/, 'Password must contain a number'),
  roll_number: z
    .string()
    .trim()
    .regex(/^\d{6}$/, 'Roll number must be exactly 6 digits (e.g. 012345)'),
  phone_number: z
    .string()
    .min(7, 'Phone number is required')
    .max(20)
    .trim()
    .regex(/^[0-9+\-\s]+$/, 'Phone number contains invalid characters'),
  program_id: z.string().min(1, 'Program is required'),
  semester_id: z.string().min(1, 'Semester is required'),
  lsc_id: z.string().min(1, 'LSC is required'),
});

export type StudentSignupInput = z.infer<typeof studentSignupSchema>;

export const studentLoginSchema = z.object({
  identifier: z.string().min(1, 'Roll number or email is required').trim(),
  password: z.string().min(1, 'Password is required').max(128),
  rememberMe: z.boolean().optional().default(false),
});

export type StudentLoginInput = z.infer<typeof studentLoginSchema>;

export const adminLoginSchema = z.object({
  email: z.string().email('Invalid email address').max(255).toLowerCase(),
  password: z.string().min(1, 'Password is required').max(128),
  rememberMe: z.boolean().optional().default(false),
});

export type AdminLoginInput = z.infer<typeof adminLoginSchema>;

export const forgotPasswordSchema = z.object({
  email: z.string().email('Invalid email address').max(255).toLowerCase(),
});

export type ForgotPasswordInput = z.infer<typeof forgotPasswordSchema>;

export const resetPasswordSchema = z.object({
  token: z.string().min(10, 'Reset token is required').max(256),
  newPassword: z
    .string()
    .min(8, 'Password must be at least 8 characters')
    .max(128)
    .regex(/[A-Za-z]/, 'Password must contain a letter')
    .regex(/[0-9]/, 'Password must contain a number'),
});

export type ResetPasswordInput = z.infer<typeof resetPasswordSchema>;

export const refreshSchema = z.object({
  refreshToken: z.string().min(1).max(512).optional(),
});

/**
 * Change password for an already-authenticated user.
 *
 * `refreshToken` is optional and only used to recognise *this* session so it can
 * be kept alive while every other device is signed out. It is never logged,
 * stored or echoed back. `.strict()` rejects unknown keys rather than letting a
 * caller smuggle in fields such as a target user id.
 */
export const changePasswordSchema = z
  .object({
    currentPassword: z.string().min(1, 'Current password is required').max(128),
    newPassword: z
      .string()
      .min(8, 'New password must be at least 8 characters')
      .max(128)
      .regex(/[A-Za-z]/, 'New password must contain a letter')
      .regex(/[0-9]/, 'New password must contain a number'),
    confirmPassword: z.string().min(1, 'Please confirm your new password').max(128),
    refreshToken: z.string().min(1).max(512).optional(),
  })
  .strict()
  .refine((data) => data.newPassword === data.confirmPassword, {
    path: ['confirmPassword'],
    message: 'Passwords do not match',
  })
  .refine((data) => data.newPassword !== data.currentPassword, {
    path: ['newPassword'],
    message: 'New password must be different from your current password',
  });

export type ChangePasswordInput = z.infer<typeof changePasswordSchema>;

export type RefreshInput = z.infer<typeof refreshSchema>;
