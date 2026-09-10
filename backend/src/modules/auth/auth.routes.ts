import { Router } from 'express';
import * as authController from './auth.controller';
import { authenticate } from '../../middleware/authenticate';
import { validate } from '../../middleware/validate';
import {
  loginRateLimiter,
  signupRateLimiter,
  refreshRateLimiter,
  forgotPasswordRateLimiter,
  passwordChangeRateLimiter,
} from '../../middleware/rateLimiter';
import {
  studentSignupSchema,
  studentLoginSchema,
  adminLoginSchema,
  forgotPasswordSchema,
  resetPasswordSchema,
  changePasswordSchema,
} from '../../validators/auth.validator';

const router = Router();

// ── Public routes ──────────────────────────────────────────────────────────

router.post('/student/signup', signupRateLimiter, validate(studentSignupSchema), authController.studentSignup);

router.post('/student/login', loginRateLimiter, validate(studentLoginSchema), authController.studentLogin);

router.post('/admin/login', loginRateLimiter, validate(adminLoginSchema), authController.adminLogin);

router.post('/refresh', refreshRateLimiter, authController.refresh);

router.post('/forgot-password', forgotPasswordRateLimiter, validate(forgotPasswordSchema), authController.forgotPassword);

router.post('/reset-password', forgotPasswordRateLimiter, validate(resetPasswordSchema), authController.resetPassword);

// ── Authenticated routes ───────────────────────────────────────────────────

router.get('/me', authenticate, authController.getMe);

// Password: requires the current password, re-verified server-side.
router.post(
  '/change-password',
  authenticate,
  passwordChangeRateLimiter,
  validate(changePasswordSchema),
  authController.changePassword,
);

// Active sessions: list the caller's own sessions and revoke one of them.
// Both endpoints scope every query to req.user.id — a client-supplied session
// id from another account is simply not found.
router.get('/sessions', authenticate, authController.getSessions);
router.delete('/sessions/:id', authenticate, authController.revokeSession);

// Single logout endpoint — role-agnostic; students use it too.
router.post('/logout', authenticate, authController.logout);
router.post('/student/logout', authenticate, authController.logout);

export default router;
