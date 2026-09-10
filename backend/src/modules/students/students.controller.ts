import type { Request, Response, NextFunction } from 'express';
import { prisma } from '../../utils/prisma';
import { createAuditLog, AUDIT_ACTIONS } from '../../utils/audit';
import { AppError } from '../../middleware/errorHandler';
import type { UserStatus } from '@prisma/client';
import type { UpdateStudentProfileInput, StudentStatusInput } from '../../validators/student.validator';


// ── Current student identity (GET /api/student/me) ───────────────────────

export async function getMe(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: {
        id: true,
        user_id: true,
        full_name: true,
        roll_number: true,
        program: { select: { id: true, name: true, code: true } },
        semester: { select: { id: true, name: true, semester_number: true } },
        lsc: { select: { id: true, name: true, code: true } },
        user: { select: { id: true, email: true, role: true, status: true } },
      },
    });

    if (!student) throw new AppError('Student profile not found', 404);

    res.json({
      success: true,
      data: {
        id: student.user_id,
        role: 'STUDENT' as const,
        fullName: student.full_name,
        rollNumber: student.roll_number,
        email: student.user.email,
        program: student.program,
        semester: student.semester,
        lsc: student.lsc,
      },
    });
  } catch (err) {
    next(err);
  }
}

// ── Profile (GET / PATCH /api/student/profile) ───────────────────────────

/**
 * The only student profile fields a student may change about themselves.
 * Roll Number, Program, Semester, LSC and email are institution-controlled.
 * Exposed to the client wholesale so the UI can decide what to render as an
 * input instead of hard-coding its own idea of what is editable.
 */
const EDITABLE_PROFILE_FIELDS = ['fullName', 'phoneNumber'] as const;

/**
 * Columns needed by the profile screen — nothing more. Password hash, tokens
 * and internal ids (student id, program/semester/lsc ids) are deliberately not
 * selected, so the profile response contains no handle a student could use to
 * address another student's record.
 */
const PROFILE_SELECT = {
  full_name: true,
  roll_number: true,
  phone_number: true,
  program: { select: { name: true, code: true } },
  semester: { select: { name: true, semester_number: true } },
  lsc: { select: { name: true, code: true } },
  user: {
    select: { email: true, status: true, created_at: true, last_login_at: true },
  },
} as const;

interface ProfileSource {
  full_name: string;
  roll_number: string;
  phone_number: string;
  program: { name: string; code: string } | null;
  semester: { name: string; semester_number: number } | null;
  lsc: { name: string; code: string } | null;
  user: {
    email: string;
    status: UserStatus;
    created_at: Date;
    last_login_at: Date | null;
  };
}

/**
 * Single response shape shared by GET and PATCH so the client always receives
 * the same object and can replace its state wholesale.
 */
function toProfileResponse(student: ProfileSource) {
  return {
    // Personal information
    fullName: student.full_name,
    rollNumber: student.roll_number,
    email: student.user.email,
    phoneNumber: student.phone_number,

    // Academic information — institution-controlled, read-only for the student
    program: student.program ? { name: student.program.name, code: student.program.code } : null,
    semester: student.semester
      ? { name: student.semester.name, semesterNumber: student.semester.semester_number }
      : null,
    lsc: student.lsc ? { name: student.lsc.name, code: student.lsc.code } : null,

    // Account information
    accountStatus: student.user.status,
    accountCreatedAt: student.user.created_at,
    lastLoginAt: student.user.last_login_at,

    // Authoritative editability contract for this endpoint.
    editableFields: [...EDITABLE_PROFILE_FIELDS],
  };
}

export async function getProfile(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    // The student is always resolved from the authenticated session — a
    // student id supplied by the browser is never read or trusted.
    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: PROFILE_SELECT,
    });

    if (!student) throw new AppError('Student profile not found', 404);

    res.json({ success: true, data: toProfileResponse(student) });
  } catch (err) {
    next(err);
  }
}

export async function updateProfile(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    // The body has already been through updateStudentProfileSchema, which is
    // strict: any field outside the whitelist (roll_number, program_id,
    // semester_id, lsc_id, email, status, role, another student's id, …) has
    // been rejected with a 400 before reaching this point. Nothing is silently
    // ignored, so a client can never believe it changed a protected value.
    const { fullName, phoneNumber } = req.body as UpdateStudentProfileInput;

    const existing = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: { id: true },
    });
    if (!existing) throw new AppError('Student profile not found', 404);

    const updated = await prisma.students.update({
      where: { user_id: req.user.id },
      data: {
        ...(fullName !== undefined ? { full_name: fullName } : {}),
        ...(phoneNumber !== undefined ? { phone_number: phoneNumber } : {}),
      },
      select: PROFILE_SELECT,
    });

    await createAuditLog({
      userId: req.user.id,
      action: AUDIT_ACTIONS.STUDENT_UPDATED,
      ipAddress: req.ip,
      deviceInfo: (req.headers['user-agent'] as string | undefined) ?? null,
      // Field names only — submitted values are never written to the audit log.
      metadata: { scope: 'self_profile', fields: Object.keys(req.body) },
    });

    res.json({ success: true, data: toProfileResponse(updated) });
  } catch (err) {
    next(err);
  }
}

// ── Courses (GET /api/student/courses) — authorized courses only ─────────

export async function getCourses(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: { id: true, program_id: true, semester_id: true },
    });
    if (!student) throw new AppError('Student profile not found', 404);

    // Courses matching the student's program + semester, plus their enrollment status.
    const courses = await prisma.courses.findMany({
      where: {
        program_id: student.program_id,
        semester_id: student.semester_id,
      },
      orderBy: { name: 'asc' },
      include: {
        student_courses: {
          where: { student_id: student.id },
          select: { id: true, status: true, assigned_at: true },
        },
      },
    });

    res.json({
      success: true,
      data: courses.map((c) => ({
        id: c.id,
        name: c.name,
        code: c.code,
        description: c.description,
        enrollment: c.student_courses[0]
          ? { status: c.student_courses[0].status, assignedAt: c.student_courses[0].assigned_at }
          : null,
      })),
    });
  } catch (err) {
    next(err);
  }
}

// ── Course selection (POST /api/student/courses/select) ──────────────────
// §12: backend must verify the student is actually authorized for the course
// (course belongs to the student's program + semester).

export async function selectCourse(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);
    const { courseId } = req.body as { courseId?: string };
    if (!courseId || typeof courseId !== 'string') throw new AppError('courseId is required', 400);

    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: { id: true, program_id: true, semester_id: true },
    });
    if (!student) throw new AppError('Student profile not found', 404);

    const course = await prisma.courses.findFirst({
      where: {
        id: courseId,
        program_id: student.program_id,
        semester_id: student.semester_id,
      },
      select: { id: true, name: true, code: true },
    });
    if (!course) throw new AppError('Course not available for your program/semester', 403);

    // Upsert ACTIVE enrollment (idempotent — no duplicate enrollments).
    const enrollment = await prisma.student_courses.upsert({
      where: { student_id_course_id: { student_id: student.id, course_id: course.id } },
      update: { status: 'ACTIVE' },
      create: { student_id: student.id, course_id: course.id, status: 'ACTIVE' },
    });

    await createAuditLog({
      userId: req.user.id,
      action: AUDIT_ACTIONS.STUDENT_UPDATED,
      ipAddress: req.ip,
      metadata: { type: 'COURSE_SELECTED', course_id: course.id },
    });

    res.json({ success: true, data: { course, enrollment } });
  } catch (err) {
    next(err);
  }
}

// ── Dashboard aggregate (GET /api/student/dashboard) ─────────────────────

export async function getDashboard(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: {
        id: true,
        full_name: true,
        roll_number: true,
        phone_number: true,
        program: { select: { id: true, name: true, code: true } },
        semester: { select: { id: true, name: true, semester_number: true } },
        lsc: { select: { id: true, name: true, code: true } },
        user: { select: { id: true, email: true, status: true, last_login_at: true } },
      },
    });
    if (!student) throw new AppError('Student profile not found', 404);

    const [enrollments, recentSessions] = await Promise.all([
      prisma.student_courses.findMany({
        where: { student_id: student.id, status: 'ACTIVE' },
        include: { course: { select: { id: true, name: true, code: true, description: true } } },
        orderBy: { assigned_at: 'desc' },
      }),
      prisma.learning_sessions.findMany({
        where: { student_id: student.id },
        orderBy: { started_at: 'desc' },
        take: 20,
        include: { course: { select: { id: true, name: true, code: true } } },
      }),
    ]);

    const courses = enrollments.map((e) => e.course);
    const latestSession = recentSessions[0] ?? null;

    // Current course = course of the most recent session; fall back to latest enrollment.
    const currentCourse = latestSession?.course ?? courses[0] ?? null;

    res.json({
      success: true,
      data: {
        student: {
          fullName: student.full_name,
          rollNumber: student.roll_number,
          email: student.user.email,
          phoneNumber: student.phone_number,
          program: student.program,
          semester: student.semester,
          lsc: student.lsc,
          lastLoginAt: student.user.last_login_at,
        },
        currentCourse,
        courses,
        progress: buildProgress(enrollments, recentSessions),
        recentActivity: recentSessions.slice(0, 5).map((s) => ({
          id: s.id,
          courseId: s.course_id,
          courseName: s.course.name,
          unitId: s.unit_id,
          chapterId: s.chapter_id,
          startedAt: s.started_at,
          endedAt: s.ended_at,
          durationSeconds: s.duration_seconds,
          completionStatus: s.completion_status,
        })),
        recentSession: latestSession,
      },
    });
  } catch (err) {
    next(err);
  }
}

// ── Progress derivation (§13: no fake values — derived from real sessions) ─

interface EnrollmentWithCourse {
  course_id: string;
  course: { name: string };
}

interface SessionRow {
  course_id: string;
  unit_id: string | null;
  completion_status: string;
}

interface ProgressRow {
  courseId: string;
  courseName: string;
  percent: number;
  units: Array<{ unitId: string; status: string; percent: number }>;
}

function buildProgress(enrollments: EnrollmentWithCourse[], sessions: SessionRow[]): ProgressRow[] {
  return enrollments.map((e) => {
    const courseSessions = sessions.filter((s) => s.course_id === e.course_id);
    const unitIds = [...new Set(courseSessions.map((s) => s.unit_id).filter((u): u is string => !!u))];

    const units = unitIds.map((unitId) => {
      const unitSessions = courseSessions.filter((s) => s.unit_id === unitId);
      const completed = unitSessions.filter((s) => s.completion_status === 'COMPLETED').length;
      const percent = unitSessions.length ? Math.round((completed / unitSessions.length) * 100) : 0;
      return {
        unitId,
        status: percent >= 100 ? 'Completed' : percent > 0 ? 'In Progress' : 'Not Started',
        percent,
      };
    });

    const percent = units.length ? Math.round(units.reduce((acc, u) => acc + u.percent, 0) / units.length) : 0;
    return {
      courseId: e.course_id,
      courseName: e.course.name,
      units,
      percent,
    };
  });
}

// ── Progress (GET /api/student/progress) ─────────────────────────────────

export async function getProgress(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: { id: true },
    });
    if (!student) throw new AppError('Student profile not found', 404);

    const [enrollments, sessions] = await Promise.all([
      prisma.student_courses.findMany({
        where: { student_id: student.id, status: 'ACTIVE' },
        include: { course: { select: { id: true, name: true } } },
        orderBy: { assigned_at: 'desc' },
      }),
      prisma.learning_sessions.findMany({
        where: { student_id: student.id },
        orderBy: { started_at: 'desc' },
        take: 200,
      }),
    ]);

    res.json({ success: true, data: buildProgress(enrollments, sessions) });
  } catch (err) {
    next(err);
  }
}

// ── Recent activity (GET /api/student/activity) ──────────────────────────

export async function getActivity(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: { id: true },
    });
    if (!student) throw new AppError('Student profile not found', 404);

    const sessions = await prisma.learning_sessions.findMany({
      where: { student_id: student.id },
      orderBy: { started_at: 'desc' },
      take: 20,
      include: { course: { select: { id: true, name: true } } },
    });

    res.json({
      success: true,
      data: sessions.map((s) => ({
        id: s.id,
        courseId: s.course_id,
        courseName: s.course.name,
        unitId: s.unit_id,
        chapterId: s.chapter_id,
        startedAt: s.started_at,
        endedAt: s.ended_at,
        durationSeconds: s.duration_seconds,
        completionStatus: s.completion_status,
      })),
    });
  } catch (err) {
    next(err);
  }
}

// ── Learning sessions (GET /api/student/sessions) ────────────────────────

export async function getSessions(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    const student = await prisma.students.findUnique({
      where: { user_id: req.user.id },
      select: { id: true },
    });
    if (!student) throw new AppError('Student profile not found', 404);

    const sessions = await prisma.learning_sessions.findMany({
      where: { student_id: student.id },
      orderBy: { started_at: 'desc' },
      take: 50,
    });

    res.json({ success: true, data: sessions });
  } catch (err) {
    next(err);
  }
}
