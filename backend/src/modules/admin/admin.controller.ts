import type { Request, Response, NextFunction } from 'express';
import { prisma } from '../../utils/prisma';
import { createAuditLog, AUDIT_ACTIONS } from '../../utils/audit';
import { AppError } from '../../middleware/errorHandler';
import type { ListStudentsQuery, UpdateStudentAcademicInput, StudentStatusInput } from '../../validators/student.validator';

// ── Current admin identity (GET /api/admin/me) ───────────────────────────

export async function getMe(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) throw new AppError('Authentication required', 401);

    const admin = await prisma.admins.findUnique({
      where: { user_id: req.user.id },
      select: {
        full_name: true,
        user: { select: { id: true, email: true, role: true, status: true, created_at: true } },
      },
    });

    if (!admin) throw new AppError('Admin profile not found', 404);

    res.json({
      success: true,
      data: {
        id: admin.user.id,
        role: 'ADMIN' as const,
        fullName: admin.full_name,
        email: admin.user.email,
        status: admin.user.status,
      },
    });
  } catch (err) {
    next(err);
  }
}

// ── Students list (pagination + search) ──────────────────────────────────

export async function listStudents(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { page, limit, search, status } = req.query as unknown as ListStudentsQuery;
    const skip = (page - 1) * limit;

    const where: Record<string, unknown> = {};
    const filters: unknown[] = [];
    if (status) filters.push({ user: { status } });
    if (search) {
      const term = { contains: search, mode: 'insensitive' as const };
      filters.push({
        OR: [
          { full_name: term },
          { roll_number: term },
          { phone_number: term },
          { user: { email: term } },
        ],
      });
    }
    if (filters.length === 1) Object.assign(where, filters[0]);
    else if (filters.length > 1) where.AND = filters;

    const [total, students] = await Promise.all([
      prisma.students.count({ where: where as never }),
      prisma.students.findMany({
        where: where as never,
        skip,
        take: limit,
        orderBy: { created_at: 'desc' },
        include: {
          user: { select: { id: true, email: true, status: true, created_at: true, last_login_at: true } },
          program: { select: { id: true, name: true, code: true } },
          semester: { select: { id: true, name: true, semester_number: true } },
          lsc: { select: { id: true, name: true, code: true } },
        },
      }),
    ]);

    res.json({
      success: true,
      data: students,
      meta: { total, page, limit, totalPages: Math.ceil(total / limit) },
    });
  } catch (err) {
    next(err);
  }
}

// ── Student by id ────────────────────────────────────────────────────────

const studentInclude = {
  user: { select: { id: true, email: true, role: true, status: true, created_at: true, last_login_at: true } },
  program: { select: { id: true, name: true, code: true } },
  semester: { select: { id: true, name: true, semester_number: true } },
  lsc: { select: { id: true, name: true, code: true } },
} as const;

export async function getStudentById(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { id } = req.params as { id: string };
    const student = await prisma.students.findUnique({ where: { id }, include: studentInclude });
    if (!student) throw new AppError('Student not found', 404);
    res.json({ success: true, data: student });
  } catch (err) {
    next(err);
  }
}

// ── Patch academic fields (program / semester / lsc) ────────────────────

export async function updateStudent(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { id } = req.params as { id: string };
    const input = req.body as UpdateStudentAcademicInput;

    const existing = await prisma.students.findUnique({ where: { id } });
    if (!existing) throw new AppError('Student not found', 404);

    await assertActiveRefs(input);

    const updated = await prisma.students.update({
      where: { id },
      data: {
        ...(input.program_id !== undefined ? { program_id: input.program_id } : {}),
        ...(input.semester_id !== undefined ? { semester_id: input.semester_id } : {}),
        ...(input.lsc_id !== undefined ? { lsc_id: input.lsc_id } : {}),
      },
      include: studentInclude,
    });

    await createAuditLog({
      userId: req.user?.id ?? null,
      action: AUDIT_ACTIONS.STUDENT_UPDATED,
      ipAddress: req.ip,
      deviceInfo: req.headers['user-agent'] ?? null,
      metadata: { student_id: id, fields: Object.keys(input) },
    });

    res.json({ success: true, data: updated });
  } catch (err) {
    next(err);
  }
}

async function assertActiveRefs(input: UpdateStudentAcademicInput): Promise<void> {
  const checks = await Promise.all([
    input.program_id
      ? prisma.programs.findFirst({ where: { id: input.program_id, status: 'ACTIVE' }, select: { id: true } })
      : Promise.resolve({ id: true }),
    input.semester_id
      ? prisma.semesters.findFirst({ where: { id: input.semester_id, status: 'ACTIVE' }, select: { id: true } })
      : Promise.resolve({ id: true }),
    input.lsc_id
      ? prisma.lscs.findFirst({ where: { id: input.lsc_id, status: 'ACTIVE' }, select: { id: true } })
      : Promise.resolve({ id: true }),
  ]);
  if (!checks[0]) throw new AppError('Invalid program', 400);
  if (!checks[1]) throw new AppError('Invalid semester', 400);
  if (!checks[2]) throw new AppError('Invalid LSC', 400);
}

// ── Suspend / reactivate a student account ───────────────────────────────

export async function setStudentStatus(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { id } = req.params as { id: string };
    const { status } = req.body as StudentStatusInput;

    const student = await prisma.students.findUnique({
      where: { id },
      include: { user: { select: { id: true, role: true } } },
    });
    if (!student) throw new AppError('Student not found', 404);
    if (student.user.role !== 'STUDENT') throw new AppError('Target user is not a student', 400);

    const updated = await prisma.$transaction(async (tx) => {
      const user = await tx.users.update({
        where: { id: student.user_id },
        data: { status },
        select: { id: true, email: true, role: true, status: true },
      });
      // Suspending kills all live sessions immediately.
      if (status !== 'ACTIVE') {
        await tx.sessions.updateMany({
          where: { user_id: student.user_id, revoked_at: null },
          data: { revoked_at: new Date() },
        });
      }
      return user;
    });

    await createAuditLog({
      userId: req.user?.id ?? null,
      action: status === 'SUSPENDED' ? AUDIT_ACTIONS.ACCOUNT_SUSPENDED : AUDIT_ACTIONS.ACCOUNT_REACTIVATED,
      ipAddress: req.ip,
      deviceInfo: req.headers['user-agent'] ?? null,
      metadata: { target_user_id: student.user_id, roll_number: student.roll_number, status },
    });

    res.json({ success: true, data: updated });
  } catch (err) {
    next(err);
  }
}

// ── Reference data (admin view — includes inactive records) ──────────────

export async function listPrograms(_req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const programs = await prisma.programs.findMany({ orderBy: { name: 'asc' } });
    res.json({ success: true, data: programs });
  } catch (err) {
    next(err);
  }
}

export async function listSemesters(_req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const semesters = await prisma.semesters.findMany({
      include: { program: { select: { id: true, name: true, code: true } } },
      orderBy: [{ program_id: 'asc' }, { semester_number: 'asc' }],
    });
    res.json({ success: true, data: semesters });
  } catch (err) {
    next(err);
  }
}

export async function listLscs(_req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const lscs = await prisma.lscs.findMany({ orderBy: { name: 'asc' } });
    res.json({ success: true, data: lscs });
  } catch (err) {
    next(err);
  }
}

export async function listCourses(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const programId = req.query.program_id as string | undefined;
    const semesterId = req.query.semester_id as string | undefined;
    const where: Record<string, string> = {};
    if (programId) where.program_id = programId;
    if (semesterId) where.semester_id = semesterId;
    const courses = await prisma.courses.findMany({
      where,
      include: {
        program: { select: { id: true, name: true, code: true } },
        semester: { select: { id: true, name: true, semester_number: true } },
      },
      orderBy: { name: 'asc' },
    });
    res.json({ success: true, data: courses });
  } catch (err) {
    next(err);
  }
}

export async function dashboardStats(_req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const [totalStudents, activeStudents, totalPrograms, totalCourses, recentAuditLogs] = await Promise.all([
      prisma.students.count(),
      prisma.users.count({ where: { role: 'STUDENT', status: 'ACTIVE' } }),
      prisma.programs.count(),
      prisma.courses.count(),
      prisma.audit_logs.findMany({ orderBy: { created_at: 'desc' }, take: 10 }),
    ]);
    res.json({
      success: true,
      data: { totalStudents, activeStudents, totalPrograms, totalCourses, recentAuditLogs },
    });
  } catch (err) {
    next(err);
  }
}
