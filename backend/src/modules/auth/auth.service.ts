import { prisma } from '../../utils/prisma';
import { hashPassword, verifyPassword, burnPasswordVerify } from '../../utils/password';
import { signAccessToken, getRefreshExpiresAt, inferRememberMe } from '../../utils/jwt';
import { generateOpaqueToken, sha256Hex } from '../../utils/tokens';
import { createAuditLog, AUDIT_ACTIONS } from '../../utils/audit';
import { isThrottled, recordFailedAttempt, clearFailedAttempts } from '../../utils/loginThrottle';
import { AppError } from '../../middleware/errorHandler';
import type { users } from '@prisma/client';
import type { StudentSignupInput, StudentLoginInput, AdminLoginInput } from '../../validators/auth.validator';

export interface RequestMeta {
  ip?: string | null;
  deviceInfo?: string | null;
  /** SHA-256 of the presented refresh token — identifies the caller's session. */
  refreshTokenHash?: string | null;
}

export interface IssuedTokens {
  accessToken: string;
  refreshToken: string;
  refreshExpiresAt: Date;
}

const INVALID_CREDENTIALS = 'Invalid credentials';
const NOT_ACTIVE = 'Account is not active';
const RESET_TOKEN_TTL_MS = 60 * 60 * 1000; // 1 hour

type SafeUser = Omit<users, 'password_hash'>;

function toSafeUser(user: users): SafeUser {
  const { password_hash: _omit, ...rest } = user;
  return rest;
}

/** Mint an opaque refresh token, persist its SHA-256 hash, return the pair. */
async function issueTokens(user: users, rememberMe: boolean, meta: RequestMeta): Promise<IssuedTokens> {
  const refreshToken = generateOpaqueToken();
  const refreshExpiresAt = getRefreshExpiresAt(rememberMe);

  await prisma.sessions.create({
    data: {
      user_id: user.id,
      refresh_token_hash: sha256Hex(refreshToken),
      device_info: meta.deviceInfo ?? null,
      ip_address: meta.ip ?? null,
      expires_at: refreshExpiresAt,
    },
  });

  const accessToken = signAccessToken({ userId: user.id, email: user.email, role: user.role });
  return { accessToken, refreshToken, refreshExpiresAt };
}

async function auditLoginFailure(meta: RequestMeta, identifier: string, userId: string | null, reason: string): Promise<void> {
  recordFailedAttempt(meta.ip ?? undefined, identifier);
  await createAuditLog({
    userId,
    action: AUDIT_ACTIONS.LOGIN_FAILED,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
    metadata: { identifier, reason },
  });
}

function assertNotThrottled(identifier: string, meta: RequestMeta): void {
  if (isThrottled(meta.ip ?? undefined, identifier)) {
    throw new AppError('Too many login attempts. Please try again after 15 minutes.', 429);
  }
}

// ── studentSignup ──────────────────────────────────────────────────────────

export async function studentSignup(input: StudentSignupInput, meta: RequestMeta) {
  const [existingEmail, existingRoll] = await Promise.all([
    prisma.users.findUnique({ where: { email: input.email }, select: { id: true } }),
    prisma.students.findUnique({ where: { roll_number: input.roll_number }, select: { id: true } }),
  ]);

  if (existingEmail) throw new AppError('Email already registered', 409);
  if (existingRoll) throw new AppError('Roll number already registered', 409);

  const [program, semester, lsc] = await Promise.all([
    prisma.programs.findFirst({ where: { id: input.program_id, status: 'ACTIVE' }, select: { id: true } }),
    prisma.semesters.findFirst({ where: { id: input.semester_id, status: 'ACTIVE' }, select: { id: true } }),
    prisma.lscs.findFirst({ where: { id: input.lsc_id, status: 'ACTIVE' }, select: { id: true } }),
  ]);
  if (!program) throw new AppError('Invalid program', 400);
  if (!semester) throw new AppError('Invalid semester', 400);
  if (!lsc) throw new AppError('Invalid LSC', 400);

  const passwordHash = await hashPassword(input.password);

  let user: users;
  let student;
  try {
    const result = await prisma.$transaction(async (tx) => {
      const createdUser = await tx.users.create({
        data: {
          email: input.email,
          password_hash: passwordHash,
          role: 'STUDENT', // role is never accepted from the client
          status: 'ACTIVE',
        },
      });

      const createdStudent = await tx.students.create({
        data: {
          user_id: createdUser.id,
          full_name: input.full_name,
          roll_number: input.roll_number,
          phone_number: input.phone_number,
          program_id: input.program_id,
          semester_id: input.semester_id,
          lsc_id: input.lsc_id,
        },
        include: {
          program: { select: { id: true, name: true, code: true } },
          semester: { select: { id: true, name: true, semester_number: true } },
          lsc: { select: { id: true, name: true, code: true } },
        },
      });

      return { createdUser, createdStudent };
    });
    user = result.createdUser;
    student = result.createdStudent;
  } catch (err) {
    // Catch the race between pre-check and insert: unique violations → 409
    if ((err as { code?: string }).code === 'P2002') {
      const target = ((err as { meta?: { target?: string } }).meta?.target ?? '').toString();
      throw new AppError(target.includes('roll_number') ? 'Roll number already registered' : 'Email already registered', 409);
    }
    throw err;
  }

  await createAuditLog({
    userId: user.id,
    action: AUDIT_ACTIONS.ACCOUNT_CREATED,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
    metadata: { role: 'STUDENT', roll_number: input.roll_number },
  });

  return { user: toSafeUser(user), student };
}

// ── studentLogin ───────────────────────────────────────────────────────────

/** Resolve a STUDENT user by roll_number OR email. Admins are never returned here. */
async function findStudentByIdentifier(identifier: string): Promise<users | null> {
  if (identifier.includes('@')) {
    return prisma.users.findFirst({ where: { email: identifier, role: 'STUDENT' } });
  }
  const student = await prisma.students.findUnique({
    where: { roll_number: identifier },
    include: { user: true },
  });
  return student?.user ?? null;
}

export async function studentLogin(input: StudentLoginInput, meta: RequestMeta) {
  assertNotThrottled(input.identifier, meta);

  const user = await findStudentByIdentifier(input.identifier);

  // Burn constant argon2 work when the account is unknown so timing does
  // not leak which roll numbers / emails exist.
  if (!user) {
    await burnPasswordVerify();
    await auditLoginFailure(meta, input.identifier, null, 'unknown_account');
    throw new AppError(INVALID_CREDENTIALS, 401);
  }

  const valid = await verifyPassword(user.password_hash, input.password);
  if (!valid) {
    await auditLoginFailure(meta, input.identifier, user.id, 'bad_password');
    throw new AppError(INVALID_CREDENTIALS, 401);
  }

  if (user.status !== 'ACTIVE') {
    await auditLoginFailure(meta, input.identifier, user.id, `status_${String(user.status).toLowerCase()}`);
    throw new AppError(NOT_ACTIVE, 403);
  }

  clearFailedAttempts(meta.ip ?? undefined, input.identifier);
  await prisma.users.update({ where: { id: user.id }, data: { last_login_at: new Date() } });

  const tokens = await issueTokens(user, input.rememberMe, meta);

  await createAuditLog({
    userId: user.id,
    action: AUDIT_ACTIONS.LOGIN_SUCCESS,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
    metadata: { method: input.identifier.includes('@') ? 'email' : 'roll_number', remember_me: input.rememberMe },
  });

  const student = await prisma.students.findUnique({
    where: { user_id: user.id },
    include: {
      program: { select: { id: true, name: true, code: true } },
      semester: { select: { id: true, name: true, semester_number: true } },
      lsc: { select: { id: true, name: true, code: true } },
    },
  });

  return { user: toSafeUser(user), student, ...tokens };
}

// ── adminLogin ─────────────────────────────────────────────────────────────

export async function adminLogin(input: AdminLoginInput, meta: RequestMeta) {
  assertNotThrottled(input.email, meta);

  const user = await prisma.users.findUnique({ where: { email: input.email } });

  // Same error shape + timing whether the email is unknown or not an admin.
  if (!user || user.role !== 'ADMIN') {
    await burnPasswordVerify();
    await auditLoginFailure(meta, input.email, user?.id ?? null, 'unknown_or_not_admin');
    throw new AppError(INVALID_CREDENTIALS, 401);
  }

  const valid = await verifyPassword(user.password_hash, input.password);
  if (!valid) {
    await auditLoginFailure(meta, input.email, user.id, 'bad_password');
    throw new AppError(INVALID_CREDENTIALS, 401);
  }

  if (user.status !== 'ACTIVE') {
    await auditLoginFailure(meta, input.email, user.id, `status_${String(user.status).toLowerCase()}`);
    throw new AppError(NOT_ACTIVE, 403);
  }

  clearFailedAttempts(meta.ip ?? undefined, input.email);
  await prisma.users.update({ where: { id: user.id }, data: { last_login_at: new Date() } });

  const tokens = await issueTokens(user, input.rememberMe, meta);

  await createAuditLog({
    userId: user.id,
    action: AUDIT_ACTIONS.LOGIN_SUCCESS,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
    metadata: { portal: 'admin', remember_me: input.rememberMe },
  });

  return { user: toSafeUser(user), ...tokens };
}

// ── refresh ────────────────────────────────────────────────────────────────
// Rotation with reuse detection: the presented token maps (by SHA-256) to a
// session row. A live row is atomically revoked (claimed) and a new session
// minted. Presenting an already-revoked token means the token leaked — we
// revoke the user's whole session family.

export async function refresh(presentedToken: string, meta: RequestMeta): Promise<IssuedTokens> {
  if (!presentedToken) throw new AppError('Refresh token is required', 401);

  const session = await prisma.sessions.findUnique({
    where: { refresh_token_hash: sha256Hex(presentedToken) },
  });
  if (!session) throw new AppError('Invalid refresh token', 401);

  // Status check BEFORE reuse handling: suspension/admin revocation also sets
  // revoked_at, and a suspended user must get 403, not a 401 reuse signal.
  const user = await prisma.users.findUnique({ where: { id: session.user_id } });
  if (!user) throw new AppError('Invalid refresh token', 401);
  if (user.status !== 'ACTIVE') {
    await revokeAllSessions(user.id);
    throw new AppError(NOT_ACTIVE, 403);
  }

  if (session.revoked_at) {
    // Valid token signature but already-consumed row → token leaked/replayed.
    await revokeAllSessions(session.user_id);
    await createAuditLog({
      userId: session.user_id,
      action: AUDIT_ACTIONS.REFRESH_TOKEN_REUSED,
      ipAddress: meta.ip,
      deviceInfo: meta.deviceInfo,
    });
    throw new AppError('Invalid refresh token', 401);
  }

  if (session.expires_at.getTime() <= Date.now()) {
    throw new AppError('Refresh token expired', 401);
  }

  // Atomic claim: if a concurrent request already rotated it, count === 0.
  const claim = await prisma.sessions.updateMany({
    where: { id: session.id, revoked_at: null },
    data: { revoked_at: new Date() },
  });
  if (claim.count === 0) {
    await revokeAllSessions(session.user_id);
    await createAuditLog({
      userId: session.user_id,
      action: AUDIT_ACTIONS.REFRESH_TOKEN_REUSED,
      ipAddress: meta.ip,
      deviceInfo: meta.deviceInfo,
    });
    throw new AppError('Invalid refresh token', 401);
  }

  const rememberMe = inferRememberMe(session.created_at, session.expires_at);
  const tokens = await issueTokens(user, rememberMe, meta);

  await createAuditLog({
    userId: user.id,
    action: AUDIT_ACTIONS.SESSION_REFRESHED,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
  });

  return tokens;
}

// ── logout ─────────────────────────────────────────────────────────────────

export async function logout(refreshToken: string | undefined, userId: string, meta: RequestMeta): Promise<void> {
  if (refreshToken) {
    await prisma.sessions.updateMany({
      where: { refresh_token_hash: sha256Hex(refreshToken), user_id: userId, revoked_at: null },
      data: { revoked_at: new Date() },
    });
  } else {
    // No token supplied → sign out everywhere.
    await revokeAllSessions(userId);
  }

  await createAuditLog({
    userId,
    action: AUDIT_ACTIONS.LOGOUT,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
    metadata: { scope: refreshToken ? 'current_session' : 'all_sessions' },
  });
}

async function revokeAllSessions(userId: string): Promise<void> {
  await prisma.sessions.updateMany({
    where: { user_id: userId, revoked_at: null },
    data: { revoked_at: new Date() },
  });
}

// ── changePassword (authenticated) ─────────────────────────────────────────
// Verifies the current password, then rotates it and revokes every session
// except the caller's current one (so other devices are forced to re-login).

export async function changePassword(
  userId: string,
  currentPassword: string,
  newPassword: string,
  currentSessionId: string | null,
  meta: RequestMeta,
): Promise<{ revokedCount: number }> {
  const user = await prisma.users.findUnique({ where: { id: userId } });
  if (!user) throw new AppError('User not found', 404);

  const valid = await verifyPassword(user.password_hash, currentPassword);
  if (!valid) {
    await createAuditLog({
      userId,
      action: AUDIT_ACTIONS.PASSWORD_CHANGED,
      ipAddress: meta.ip,
      deviceInfo: meta.deviceInfo,
      metadata: { result: 'rejected_bad_current_password' },
    });
    throw new AppError('Current password is incorrect', 401);
  }

  if (await verifyPassword(user.password_hash, newPassword)) {
    throw new AppError('New password must be different from the current password', 400);
  }

  const passwordHash = await hashPassword(newPassword);

  // Resolve the caller's current session (by presented refresh token hash) so
  // it survives the revocation sweep below.
  let keepSessionId: string | null = null;
  if (currentSessionId) {
    keepSessionId = currentSessionId;
  } else if (meta.refreshTokenHash) {
    const row = await prisma.sessions.findUnique({
      where: { refresh_token_hash: meta.refreshTokenHash },
      select: { id: true },
    });
    keepSessionId = row?.id ?? null;
  }

  const revoked = await prisma.$transaction(async (tx) => {
    await tx.users.update({ where: { id: userId }, data: { password_hash: passwordHash } });
    return tx.sessions.updateMany({
      where: {
        user_id: userId,
        revoked_at: null,
        expires_at: { gt: new Date() },
        ...(keepSessionId ? { NOT: { id: keepSessionId } } : {}),
      },
      data: { revoked_at: new Date() },
    });
  });

  await createAuditLog({
    userId,
    action: AUDIT_ACTIONS.PASSWORD_CHANGED,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
    metadata: { result: 'success', revoked_sessions: revoked.count },
  });

  return { revokedCount: revoked.count };
}

// ── listSessions / revokeSession (authenticated) ───────────────────────────
// Device/session management for the profile Security section. Only metadata
// (device, ip, timestamps) is ever returned — never token material.

export interface SessionView {
  id: string;
  deviceInfo: string | null;
  ipAddress: string | null;
  createdAt: Date;
  expiresAt: Date;
  revokedAt: Date | null;
  isCurrent: boolean;
  isActive: boolean;
}

export async function listSessions(userId: string, currentSessionId: string | null): Promise<SessionView[]> {
  const rows = await prisma.sessions.findMany({
    where: { user_id: userId },
    // DESC + NULLS FIRST (Postgres default) → active sessions before revoked.
    orderBy: [{ revoked_at: 'desc' }, { created_at: 'desc' }],
    take: 50,
  });

  const now = new Date();
  return rows.map((s) => ({
    id: s.id,
    deviceInfo: s.device_info,
    ipAddress: s.ip_address,
    createdAt: s.created_at,
    expiresAt: s.expires_at,
    revokedAt: s.revoked_at,
    isCurrent: currentSessionId === s.id,
    isActive: !s.revoked_at && s.expires_at.getTime() > now.getTime(),
  }));
}

export async function revokeSession(userId: string, sessionId: string, meta: RequestMeta): Promise<void> {
  // updateMany + user_id scoping: another user's session id can never be revoked.
  const claim = await prisma.sessions.updateMany({
    where: { id: sessionId, user_id: userId, revoked_at: null },
    data: { revoked_at: new Date() },
  });
  if (claim.count === 0) {
    // Same error whether it doesn't exist, belongs to someone else, or is
    // already revoked — never leak which.
    throw new AppError('Session not found or already signed out', 404);
  }

  await createAuditLog({
    userId,
    action: AUDIT_ACTIONS.SESSION_REVOKED,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
    metadata: { session_id: sessionId },
  });
}

// ── forgotPassword ─────────────────────────────────────────────────────────

export async function forgotPassword(email: string, meta: RequestMeta): Promise<{ rawToken: string | null }> {
  const user = await prisma.users.findUnique({ where: { email }, select: { id: true } });
  if (!user) {
    // Caller responds with the same generic message — existence is never revealed.
    return { rawToken: null };
  }

  const rawToken = generateOpaqueToken();
  await prisma.password_resets.create({
    data: {
      user_id: user.id,
      token_hash: sha256Hex(rawToken),
      expires_at: new Date(Date.now() + RESET_TOKEN_TTL_MS),
    },
  });

  await createAuditLog({
    userId: user.id,
    action: AUDIT_ACTIONS.PASSWORD_RESET_REQUESTED,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
  });

  // No email service in Phase 1 — controller surfaces the token in dev mode only.
  return { rawToken };
}

// ── resetPassword ──────────────────────────────────────────────────────────

export async function resetPassword(rawToken: string, newPassword: string, meta: RequestMeta): Promise<void> {
  const reset = await prisma.password_resets.findUnique({
    where: { token_hash: sha256Hex(rawToken) },
  });
  if (!reset || reset.used_at || reset.expires_at.getTime() <= Date.now()) {
    throw new AppError('Invalid or expired reset token', 400);
  }

  const passwordHash = await hashPassword(newPassword);

  await prisma.$transaction(async (tx) => {
    // Atomic single-use claim (guards against parallel replays of the token).
    const claim = await tx.password_resets.updateMany({
      where: { id: reset.id, used_at: null },
      data: { used_at: new Date() },
    });
    if (claim.count === 0) throw new AppError('Invalid or expired reset token', 400);

    await tx.users.update({ where: { id: reset.user_id }, data: { password_hash: passwordHash } });
    // Password reset invalidates every active session.
    await tx.sessions.updateMany({
      where: { user_id: reset.user_id, revoked_at: null },
      data: { revoked_at: new Date() },
    });
  });

  await createAuditLog({
    userId: reset.user_id,
    action: AUDIT_ACTIONS.PASSWORD_RESET,
    ipAddress: meta.ip,
    deviceInfo: meta.deviceInfo,
  });
}

// ── getMe ──────────────────────────────────────────────────────────────────

export async function getMe(userId: string) {
  const user = await prisma.users.findUnique({
    where: { id: userId },
    select: { id: true, email: true, role: true, status: true, created_at: true, last_login_at: true },
  });
  if (!user) throw new AppError('User not found', 404);

  if (user.role === 'STUDENT') {
    const student = await prisma.students.findUnique({
      where: { user_id: userId },
      include: {
        program: { select: { id: true, name: true, code: true } },
        semester: { select: { id: true, name: true, semester_number: true } },
        lsc: { select: { id: true, name: true, code: true } },
      },
    });
    return { user, student };
  }

  if (user.role === 'ADMIN') {
    const admin = await prisma.admins.findUnique({
      where: { user_id: userId },
      select: { id: true, full_name: true, created_at: true },
    });
    return { user, admin };
  }

  return { user };
}
