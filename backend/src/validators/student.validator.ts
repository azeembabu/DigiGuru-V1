import { z } from 'zod';

/** Digits with an optional leading `+`, spaces and dashes. Mirrored by the
 *  student-app `profileSchema` so client and server agree on what is valid. */
const PHONE_REGEX = /^\+?[0-9][0-9\s-]{5,18}$/;

/**
 * Student-side self-service profile update.
 *
 * `.strict()` is deliberate and load-bearing: Roll Number, Program, Semester,
 * LSC, email and account status are institution-controlled. A request that
 * carries any of them (or any other unrecognised key) is rejected outright with
 * 400 — we never silently drop a field and report success, because that would
 * leave the client believing a protected value had been changed.
 */
export const updateStudentProfileSchema = z
  .object({
    fullName: z
      .string()
      .trim()
      .min(2, 'Full name must be at least 2 characters')
      .max(100, 'Full name must be at most 100 characters'),
    phoneNumber: z.string().trim().regex(PHONE_REGEX, 'Enter a valid phone number'),
  })
  .partial()
  .strict()
  .refine((data) => Object.keys(data).length > 0, {
    message: 'At least one field is required',
  });

export type UpdateStudentProfileInput = z.infer<typeof updateStudentProfileSchema>;

/** Admin-side academic field patch (program/semester/lsc). */
export const updateStudentAcademicSchema = z
  .object({
    program_id: z.string().min(1).optional(),
    semester_id: z.string().min(1).optional(),
    lsc_id: z.string().min(1).optional(),
  })
  .refine((data) => Object.keys(data).length > 0, { message: 'At least one field is required' });

export type UpdateStudentAcademicInput = z.infer<typeof updateStudentAcademicSchema>;

/** Admin-side account status change (suspend / reactivate). */
export const studentStatusSchema = z.object({
  status: z.enum(['ACTIVE', 'SUSPENDED', 'INACTIVE']),
});

export type StudentStatusInput = z.infer<typeof studentStatusSchema>;

/** Admin student-list query (pagination + search). */
export const listStudentsQuerySchema = z.object({
  page: z.coerce.number().int().min(1).default(1),
  limit: z.coerce.number().int().min(1).max(100).default(20),
  search: z.string().trim().max(255).optional(),
  status: z.enum(['ACTIVE', 'SUSPENDED', 'INACTIVE']).optional(),
});

export type ListStudentsQuery = z.infer<typeof listStudentsQuerySchema>;

/** Student course selection — only the course id comes from the client. */
export const selectCourseSchema = z.object({
  courseId: z.string().min(1),
});

export type SelectCourseInput = z.infer<typeof selectCourseSchema>;
