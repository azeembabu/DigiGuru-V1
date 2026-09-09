import express from 'express';
import helmet from 'helmet';
import cors from 'cors';
import cookieParser from 'cookie-parser';
import { env } from './config/env';
import { apiRateLimiter } from './middleware/rateLimiter';
import { errorHandler, notFoundHandler } from './middleware/errorHandler';

import authRoutes from './modules/auth/auth.routes';
import studentRoutes from './modules/students/students.routes';
import adminRoutes from './modules/admin/admin.routes';
import referenceRoutes from './modules/references/references.routes';

const app = express();

// ── Security headers ───────────────────────────────────────────────────────
app.use(helmet());

// ── CORS ───────────────────────────────────────────────────────────────────
app.use(
  cors({
    origin: env.CORS_ORIGINS,
    credentials: true,
    methods: ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'],
    allowedHeaders: ['Content-Type', 'Authorization'],
  }),
);

// ── Body parsing & cookies ─────────────────────────────────────────────────
app.use(express.json({ limit: '1mb' }));
app.use(express.urlencoded({ extended: true }));
app.use(cookieParser());

// ── Global rate limiting ───────────────────────────────────────────────────
app.use(apiRateLimiter);

// ── Health check ───────────────────────────────────────────────────────────
app.get('/health', (_req, res) => {
  res.json({ success: true, message: 'Digi Guru API is running', timestamp: new Date().toISOString() });
});

// ── Route mounting ─────────────────────────────────────────────────────────
// Prefix from env (default /api) — routes below are relative to it.
const prefix = env.API_PREFIX; // e.g. /api or /api/v1

app.use(`${prefix}/auth`, authRoutes);
app.use(`${prefix}/student`, studentRoutes);
app.use(`${prefix}/admin`, adminRoutes);
// Public read-only reference data for signup dropdowns:
// GET {prefix}/programs | /semesters | /lscs
app.use(prefix, referenceRoutes);

// Also mount without prefix for backwards compatibility when API_PREFIX=/api
if (prefix !== '/api') {
  app.use('/api/auth', authRoutes);
  app.use('/api/student', studentRoutes);
  app.use('/api/admin', adminRoutes);
  app.use('/api', referenceRoutes);
}

// ── 404 handler ────────────────────────────────────────────────────────────
app.use(notFoundHandler);

// ── Central error handler (must be last) ───────────────────────────────────
app.use(errorHandler);

export default app;
