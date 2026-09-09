import type { Request, Response, NextFunction } from 'express';
import * as authService from './auth.service';
import { setAuthCookies, clearAuthCookies, REFRESH_TOKEN_COOKIE } from '../../utils/cookies';
import { isProduction } from '../../config/env';

function getMeta(req: Request): authService.RequestMeta {
  return {
    ip: req.ip ?? req.socket.remoteAddress ?? null,
    deviceInfo: (req.headers['user-agent'] as string | undefined) ?? null,
  };
}

function sendTokens(res: Response, tokens: authService.IssuedTokens, extra: Record<string, unknown>): void {
  setAuthCookies(res, tokens.accessToken, tokens.refreshToken, tokens.refreshExpiresAt);
  // Scaffold convention: tokens also in the JSON body so non-browser clients
  // can use Authorization: Bearer. No tokens are ever placed in URLs.
  res.json({ success: true, data: { ...tokens, ...extra } });
}

// ── POST /api/auth/student/signup ────────────────────────────────────────

export async function studentSignup(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const result = await authService.studentSignup(req.body, getMeta(req));
    res.status(201).json({ success: true, data: result });
  } catch (err) {
    next(err);
  }
}

// ── POST /api/auth/student/login ─────────────────────────────────────────

export async function studentLogin(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const result = await authService.studentLogin(req.body, getMeta(req));
    sendTokens(res, result, { user: result.user, student: result.student });
  } catch (err) {
    next(err);
  }
}

// ── POST /api/auth/admin/login ───────────────────────────────────────────

export async function adminLogin(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const result = await authService.adminLogin(req.body, getMeta(req));
    sendTokens(res, result, { user: result.user });
  } catch (err) {
    next(err);
  }
}

// ── POST /api/auth/refresh ───────────────────────────────────────────────

export async function refresh(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const presented =
      (req.body?.refreshToken as string | undefined) ??
      (req.cookies?.[REFRESH_TOKEN_COOKIE] as string | undefined);

    if (!presented) {
      res.status(400).json({ success: false, error: 'Refresh token is required' });
      return;
    }

    const tokens = await authService.refresh(presented, getMeta(req));
    sendTokens(res, tokens, {});
  } catch (err) {
    next(err);
  }
}

// ── POST /api/auth/logout ────────────────────────────────────────────────

export async function logout(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) {
      res.status(401).json({ success: false, error: 'Authentication required' });
      return;
    }
    const refreshToken =
      (req.body?.refreshToken as string | undefined) ??
      (req.cookies?.[REFRESH_TOKEN_COOKIE] as string | undefined);

    await authService.logout(refreshToken, req.user.id, getMeta(req));
    clearAuthCookies(res);
    res.json({ success: true, data: { message: 'Logged out successfully' } });
  } catch (err) {
    next(err);
  }
}

// ── POST /api/auth/forgot-password ───────────────────────────────────────

export async function forgotPassword(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { email } = req.body as { email: string };
    const { rawToken } = await authService.forgotPassword(email, getMeta(req));

    // Always the same generic message — do not reveal whether the email exists.
    // Dev-only convenience: no email service in Phase 1, so surface the token.
    res.json({
      success: true,
      data: {
        message: 'If an account with that email exists, a password reset link has been sent',
        ...(!isProduction && rawToken ? { resetToken: rawToken } : {}),
      },
    });
  } catch (err) {
    next(err);
  }
}

// ── POST /api/auth/reset-password ────────────────────────────────────────

export async function resetPassword(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const { token, newPassword } = req.body as { token: string; newPassword: string };
    await authService.resetPassword(token, newPassword, getMeta(req));
    clearAuthCookies(res);
    res.json({ success: true, data: { message: 'Password has been reset successfully' } });
  } catch (err) {
    next(err);
  }
}

// ── GET /api/auth/me ─────────────────────────────────────────────────────

export async function getMe(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    if (!req.user) {
      res.status(401).json({ success: false, error: 'Authentication required' });
      return;
    }
    const data = await authService.getMe(req.user.id);
    res.json({ success: true, data });
  } catch (err) {
    next(err);
  }
}
