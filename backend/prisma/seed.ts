/**
 * Digi Guru - Database Seed (Phase 1)
 *
 * Idempotent: every row is upserted on its natural key, so this script can be
 * re-run any number of times without creating duplicates.
 *
 * Run via:  npx prisma db seed
 */
import { PrismaClient, Status, Role, UserStatus } from '@prisma/client';
import * as argon2 from 'argon2';

const prisma = new PrismaClient();

/** Same argon2id parameters as src/utils/password.ts (inlined so the seed
 *  stays self-contained and does not cross the tsconfig rootDir boundary). */
async function hashPassword(plain: string): Promise<string> {
  return argon2.hash(plain, {
    type: argon2.argon2id,
    memoryCost: 19 * 1024, // 19 MiB
    timeCost: 2,
    parallelism: 1,
  });
}

const DEMO_ADMIN_PASSWORD = 'Admin@12345';
const DEMO_STUDENT_PASSWORD = 'Student@12345';

const PROGRAMS = [
  {
    code: 'BA-MAL',
    name: 'BA Malayalam',
    description: 'Bachelor of Arts in Malayalam',
  },
  {
    code: 'BA-ENG',
    name: 'BA English',
    description: 'Bachelor of Arts in English',
  },
  {
    code: 'BCOM-FIN',
    name: 'BCom Finance',
    description: 'Bachelor of Commerce in Finance',
  },
];

const LSCS = [
  { code: 'LSC-PLKD', name: 'Palakkad LSC', location: 'Palakkad, Kerala' },
  { code: 'LSC-TSR', name: 'Thrissur LSC', location: 'Thrissur, Kerala' },
  { code: 'LSC-MLPM', name: 'Malappuram LSC', location: 'Malappuram, Kerala' },
];

const SEMESTERS_PER_PROGRAM = 6;

const BA_MALAYLAM_COURSES: Array<{
  semesterNumber: number;
  code: string;
  name: string;
  description: string;
}> = [
  { semesterNumber: 1, code: 'MAL101', name: 'Introduction to Malayalam Literature', description: 'Overview of Malayalam literary history and major movements' },
  { semesterNumber: 1, code: 'MAL102', name: 'Malayalam Grammar I', description: 'Phonology, morphology and syntax fundamentals' },
  { semesterNumber: 1, code: 'MAL103', name: 'Modern Malayalam Prose', description: 'Prose writing from the 19th century to the present' },
  { semesterNumber: 2, code: 'MAL201', name: 'Classical Malayalam Poetry', description: 'Pattu and modern poetic traditions' },
  { semesterNumber: 2, code: 'MAL202', name: 'Malayalam Grammar II', description: 'Advanced grammar and language variation' },
];

const STUDENTS = [
  {
    email: 'student001@digiguru.local',
    fullName: 'Arun Krishna',
    rollNumber: 'STUDENT001',
    phoneNumber: '+919847000001',
    programCode: 'BA-MAL',
    semesterNumber: 1,
    lscCode: 'LSC-PLKD',
  },
  {
    email: 'student002@digiguru.local',
    fullName: 'Meera Nair',
    rollNumber: 'STUDENT002',
    phoneNumber: '+919847000002',
    programCode: 'BA-MAL',
    semesterNumber: 2,
    lscCode: 'LSC-PLKD',
  },
];

async function seedPrograms(): Promise<void> {
  for (const program of PROGRAMS) {
    await prisma.programs.upsert({
      where: { code: program.code },
      create: {
        code: program.code,
        name: program.name,
        description: program.description,
        status: Status.ACTIVE,
      },
      update: {
        name: program.name,
        description: program.description,
        status: Status.ACTIVE,
      },
    });
  }
}

async function seedSemesters(): Promise<Map<string, string>> {
  const idByProgramSemester = new Map<string, string>();
  const programs = await prisma.programs.findMany({
    where: { code: { in: PROGRAMS.map((p) => p.code) } },
  });

  for (const program of programs) {
    for (let n = 1; n <= SEMESTERS_PER_PROGRAM; n += 1) {
      const semester = await prisma.semesters.upsert({
        where: {
          program_id_semester_number: { program_id: program.id, semester_number: n },
        },
        create: {
          program_id: program.id,
          semester_number: n,
          name: `Semester ${n}`,
          status: Status.ACTIVE,
        },
        update: {
          name: `Semester ${n}`,
          status: Status.ACTIVE,
        },
      });
      idByProgramSemester.set(`${program.code}#${n}`, semester.id);
    }
  }
  return idByProgramSemester;
}

async function seedLscs(): Promise<void> {
  for (const lsc of LSCS) {
    await prisma.lscs.upsert({
      where: { code: lsc.code },
      create: { code: lsc.code, name: lsc.name, location: lsc.location, status: Status.ACTIVE },
      update: { name: lsc.name, location: lsc.location, status: Status.ACTIVE },
    });
  }
}

async function seedAdmin(): Promise<void> {
  const passwordHash = await hashPassword(DEMO_ADMIN_PASSWORD);
  const user = await prisma.users.upsert({
    where: { email: 'admin@digiguru.local' },
    create: {
      email: 'admin@digiguru.local',
      password_hash: passwordHash,
      role: Role.ADMIN,
      status: UserStatus.ACTIVE,
    },
    update: {
      password_hash: passwordHash,
      role: Role.ADMIN,
      status: UserStatus.ACTIVE,
    },
  });

  await prisma.admins.upsert({
    where: { user_id: user.id },
    create: { user_id: user.id, full_name: 'System Admin' },
    update: { full_name: 'System Admin' },
  });
}

async function seedStudents(
  semesterIdByProgramSemester: Map<string, string>,
): Promise<void> {
  const [programs, lscs] = await Promise.all([
    prisma.programs.findMany({ where: { code: { in: STUDENTS.map((s) => s.programCode) } } }),
    prisma.lscs.findMany({ where: { code: { in: STUDENTS.map((s) => s.lscCode) } } }),
  ]);
  const programId = new Map(programs.map((p) => [p.code, p.id]));
  const lscId = new Map(lscs.map((l) => [l.code, l.id]));
  const passwordHash = await hashPassword(DEMO_STUDENT_PASSWORD);

  for (const student of STUDENTS) {
    const user = await prisma.users.upsert({
      where: { email: student.email },
      create: {
        email: student.email,
        password_hash: passwordHash,
        role: Role.STUDENT,
        status: UserStatus.ACTIVE,
      },
      update: {
        password_hash: passwordHash,
        role: Role.STUDENT,
        status: UserStatus.ACTIVE,
      },
    });

    const semesterId = semesterIdByProgramSemester.get(
      `${student.programCode}#${student.semesterNumber}`,
    );
    const studentProgramId = programId.get(student.programCode);
    const studentLscId = lscId.get(student.lscCode);
    if (!semesterId || !studentProgramId || !studentLscId) {
      throw new Error(
        `Seed integrity error: missing reference for ${student.rollNumber} ` +
          `(semester=${semesterId}, program=${studentProgramId}, lsc=${studentLscId})`,
      );
    }

    await prisma.students.upsert({
      where: { roll_number: student.rollNumber },
      create: {
        user_id: user.id,
        full_name: student.fullName,
        roll_number: student.rollNumber,
        phone_number: student.phoneNumber,
        program_id: studentProgramId,
        semester_id: semesterId,
        lsc_id: studentLscId,
      },
      update: {
        user_id: user.id,
        full_name: student.fullName,
        phone_number: student.phoneNumber,
        program_id: studentProgramId,
        semester_id: semesterId,
        lsc_id: studentLscId,
      },
    });
  }
}

async function seedCourses(
  semesterIdByProgramSemester: Map<string, string>,
): Promise<void> {
  const program = await prisma.programs.findUnique({ where: { code: 'BA-MAL' } });
  if (!program) {
    throw new Error('Seed integrity error: BA-MAL program missing');
  }

  for (const course of BA_MALAYLAM_COURSES) {
    const semesterId = semesterIdByProgramSemester.get(`BA-MAL#${course.semesterNumber}`);
    if (!semesterId) {
      throw new Error(`Seed integrity error: BA-MAL semester ${course.semesterNumber} missing`);
    }
    await prisma.courses.upsert({
      where: {
        program_id_semester_id_code: {
          program_id: program.id,
          semester_id: semesterId,
          code: course.code,
        },
      },
      create: {
        program_id: program.id,
        semester_id: semesterId,
        code: course.code,
        name: course.name,
        description: course.description,
      },
      update: { name: course.name, description: course.description },
    });
  }
}

async function logCounts(): Promise<void> {
  const [users, students, admins, programs, semesters, lscs, courses, sessions, passwordResets, auditLogs] =
    await Promise.all([
      prisma.users.count(),
      prisma.students.count(),
      prisma.admins.count(),
      prisma.programs.count(),
      prisma.semesters.count(),
      prisma.lscs.count(),
      prisma.courses.count(),
      prisma.sessions.count(),
      prisma.password_resets.count(),
      prisma.audit_logs.count(),
    ]);

  const rows: Array<[string, number]> = [
    ['users', users],
    ['students', students],
    ['admins', admins],
    ['programs', programs],
    ['semesters', semesters],
    ['lscs', lscs],
    ['courses', courses],
    ['sessions', sessions],
    ['password_resets', passwordResets],
    ['audit_logs', auditLogs],
  ];
  for (const [table, count] of rows) {
    console.log(`seed:${table}=${count}`);
  }
}

async function main(): Promise<void> {
  console.log('Seeding Digi Guru database...');
  await seedPrograms();
  const semesterIds = await seedSemesters();
  await seedLscs();
  await seedAdmin();
  await seedStudents(semesterIds);
  await seedCourses(semesterIds);
  await logCounts();
  console.log('Seed complete.');
}

main()
  .catch((error: unknown) => {
    console.error('Seed failed:', error);
    process.exitCode = 1;
  })
  .finally(async () => {
    await prisma.$disconnect();
  });
