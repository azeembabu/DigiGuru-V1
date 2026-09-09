import { z } from 'zod';

// ── Admin Login ───────────────────────────────────────────────────────────────

export const adminLoginSchema = z.object({
  identifier: z
    .string()
    .min(1, 'Admin ID or email is required')
    .max(254, 'Too long'),
  password: z
    .string()
    .min(1, 'Password is required')
    .min(8, 'Password must be at least 8 characters')
    .max(128, 'Password is too long'),
});

export type AdminLoginInput = z.infer<typeof adminLoginSchema>;

// ── Program ─────────────────────────────────────────────────────────────────

export const programSchema = z.object({
  name: z.string().min(1, 'Program name is required').max(120),
  code: z
    .string()
    .min(1, 'Code is required')
    .max(30)
    .regex(/^[A-Z0-9_-]+$/i, 'Code must be alphanumeric with - or _'),
  description: z.string().max(500).optional().or(z.literal('')),
  status: z.enum(['ACTIVE', 'INACTIVE']).default('ACTIVE'),
});

export type ProgramInput = z.infer<typeof programSchema>;

// ── Semester ────────────────────────────────────────────────────────────────

export const semesterSchema = z.object({
  program_id: z.string().min(1, 'Program is required'),
  semester_number: z.coerce.number().int().min(1).max(12),
  name: z.string().min(1, 'Semester name is required').max(80),
  status: z.enum(['ACTIVE', 'INACTIVE']).default('ACTIVE'),
});

export type SemesterInput = z.infer<typeof semesterSchema>;

// ── LSC ─────────────────────────────────────────────────────────────────────

export const lscSchema = z.object({
  name: z.string().min(1, 'LSC name is required').max(120),
  code: z
    .string()
    .min(1, 'Code is required')
    .max(30)
    .regex(/^[A-Z0-9_-]+$/i, 'Code must be alphanumeric with - or _'),
  location: z.string().max(200).optional().or(z.literal('')),
  status: z.enum(['ACTIVE', 'INACTIVE']).default('ACTIVE'),
});

export type LscInput = z.infer<typeof lscSchema>;
