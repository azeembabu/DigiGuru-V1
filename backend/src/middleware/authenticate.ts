import type { Request, Response, NextFunction } from 'express';
import { verifyAccessToken } from '../utils/jwt';
import { prisma } from '../utils/prisma';
import { ACCESS_TOKEN_COOKIE } from '../utils/cookies';

export async function authenticate(req: Request, res: Response, next: NextFunction): Promise<void> {
  try {
    const token = extractToken(req);
    if (!token) {
      res.status(401).json({ success: false, error: 'Authentication required' });
      return;
    }

    let payload: { userId: string; email: string; role: string; sid?: string };
    try {
      payload = verifyAccessToken(token);
    } catch {
      res.status(401).json({ success: false, error: 'Invalid or expired token' });
      return;
    }

    // Verify user still exists and is ACTIVE
    const user = await prisma.users.findUnique({
      where: { id: payload.userId },
      select: { id: true, email: true, role: true, status: true },
    });

    if (!user) {
      res.status(401).json({ success: false, error: 'User not found' });
      return;
    }

    if (user.status !== 'ACTIVE') {
      res.status(403).json({ success: false, error: 'Account is not active' });
      return;
    }

    // The access token names the session it was minted for, so a session that
    // has since been revoked (signed out on another device, revoked from the
    // Active Sessions list, or invalidated by a password change) stops working
    // immediately instead of surviving until the token expires.
    // Tokens minted before `sid` existed are still accepted.
    if (payload.sid) {
      const session = await prisma.sessions.findFirst({
        where: {
          id: payload.sid,
          user_id: user.id,
          revoked_at: null,
          expires_at: { gt: new Date() },
        },
        select: { id: true },
      });
      if (!session) {
        res.status(401).json({ success: false, error: 'Session has ended. Please sign in again.' });
        return;
      }
    }

    req.user = {
      id: user.id,
      email: user.email,
      role: user.role,
      ...(payload.sid ? { sessionId: payload.sid } : {}),
    };

    next();
  } catch (err) {
    next(err);
  }
}

function extractToken(req: Request): string | null {
  // 1) Authorization: Bearer <token>
  const authHeader = req.headers.authorization;
  if (authHeader?.startsWith('Bearer ')) {
    return authHeader.slice(7).trim();
  }
  // 2) HttpOnly cookie fallback
  const cookieToken = req.cookies?.[ACCESS_TOKEN_COOKIE] as string | undefined;
  if (cookieToken) return cookieToken;

  return null;
}

export default authenticate;
