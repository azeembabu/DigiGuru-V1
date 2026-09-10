import { prisma } from './prisma';

/**
 * Canonical audit action names (Phase 1 requirement list plus a few
 * operational extras). Never store secrets (passwords/tokens) in metadata.
 */
export const AUDIT_ACTIONS = {
  LOGIN_SUCCESS: 'LOGIN_SUCCESS',
  LOGIN_FAILED: 'LOGIN_FAILED',
  LOGOUT: 'LOGOUT',
  ACCOUNT_CREATED: 'ACCOUNT_CREATED',
  PASSWORD_RESET: 'PASSWORD_RESET',
  PASSWORD_RESET_REQUESTED: 'PASSWORD_RESET_REQUESTED',
  PASSWORD_CHANGED: 'PASSWORD_CHANGED',
  PASSWORD_CHANGE_FAILED: 'PASSWORD_CHANGE_FAILED',
  SESSION_REVOKED: 'SESSION_REVOKED',
  ACCOUNT_SUSPENDED: 'ACCOUNT_SUSPENDED',
  ACCOUNT_REACTIVATED: 'ACCOUNT_REACTIVATED',
  REFRESH_TOKEN_REUSED: 'REFRESH_TOKEN_REUSED',
  SESSION_REFRESHED: 'SESSION_REFRESHED',
  STUDENT_UPDATED: 'STUDENT_UPDATED',
} as const;

export type AuditAction = (typeof AUDIT_ACTIONS)[keyof typeof AUDIT_ACTIONS];

export interface AuditLogParams {
  userId?: string | null;
  action: AuditAction;
  ipAddress?: string | null;
  deviceInfo?: string | null;
  metadata?: Record<string, unknown> | null;
}

export async function createAuditLog(params: AuditLogParams): Promise<void> {
  try {
    await prisma.audit_logs.create({
      data: {
        user_id: params.userId ?? null,
        action: params.action,
        ip_address: params.ipAddress ?? null,
        device_info: params.deviceInfo ?? null,
        metadata: params.metadata ? (params.metadata as never) : undefined,
      },
    });
  } catch (err) {
    // Audit logging must never break the main flow — log and continue.
    // Error text is logged, never any request payload.
    console.error('[audit] failed to create audit log:', err instanceof Error ? err.message : err);
  }
}
