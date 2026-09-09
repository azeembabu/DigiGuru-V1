import type { Request, Response, NextFunction } from 'express';
import { prisma } from '../../utils/prisma';

/**
 * Read-only reference data for the student signup form dropdowns.
 * PUBLIC by design (documented decision): programs/semesters/lscs are
 * non-sensitive catalogue data needed BEFORE signup. Only ACTIVE rows are
 * exposed, and only id/name/code columns — nothing else.
 */

export async function listPrograms(_req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const programs = await prisma.programs.findMany({
      where: { status: 'ACTIVE' },
      select: { id: true, name: true, code: true },
      orderBy: { name: 'asc' },
    });
    res.json({ success: true, data: programs });
  } catch (err) {
    next(err);
  }
}

export async function listSemesters(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const programId = req.query.program_id as string | undefined;
    const semesters = await prisma.semesters.findMany({
      where: { status: 'ACTIVE', ...(programId ? { program_id: programId } : {}) },
      select: { id: true, name: true, semester_number: true, program_id: true },
      orderBy: [{ program_id: 'asc' }, { semester_number: 'asc' }],
    });
    res.json({ success: true, data: semesters });
  } catch (err) {
    next(err);
  }
}

export async function listLscs(_req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const lscs = await prisma.lscs.findMany({
      where: { status: 'ACTIVE' },
      select: { id: true, name: true, code: true, location: true },
      orderBy: { name: 'asc' },
    });
    res.json({ success: true, data: lscs });
  } catch (err) {
    next(err);
  }
}
