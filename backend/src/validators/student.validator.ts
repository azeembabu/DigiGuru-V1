import { z } from 'zod';

/**
 * Student-side self-service profile update — the ONLY fields a Student may
 * change. `.strict()` makes any other key (roll_number, program_id,
 * semester_id, lsc_id, role, status, user_id, …) a validation error, so
 * unauthorized fields are rejected — never silently accepted or ignored.
 */
export const updateStudentProfileSchema = z
  .object({
    full_name: z.string().min(2).max(100).optional(),
    phone_number: z
      .string()
      .min(7)
      .max(20)
      .trim()
      .regex(/^[0-9+\-\s]+$/, 'Phone number contains invalid characters')
      .optional(),
  })
  .strict()
  .refine((data) => Object.keys(data).length > 0, { message: 'At least one field is required' });

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
