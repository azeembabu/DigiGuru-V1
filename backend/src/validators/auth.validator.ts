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

export type RefreshInput = z.infer<typeof refreshSchema>;
