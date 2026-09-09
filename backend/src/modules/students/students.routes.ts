import { Router } from 'express';
import * as studentsController from './students.controller';
import { authenticate } from '../../middleware/authenticate';
import { requireRole } from '../../middleware/authorize';
import { validate } from '../../middleware/validate';
import { updateStudentProfileSchema, selectCourseSchema } from '../../validators/student.validator';

const router = Router();

// All student routes require a valid JWT and STUDENT role.
// Data isolation: controllers always derive the student from req.user.id —
// client-supplied ids are never trusted.

router.use(authenticate, requireRole('STUDENT'));

router.get('/me', studentsController.getMe);
router.get('/profile', studentsController.getProfile);
router.patch('/profile', validate(updateStudentProfileSchema), studentsController.updateProfile);
router.get('/courses', studentsController.getCourses);
router.post('/courses/select', validate(selectCourseSchema), studentsController.selectCourse);

// ── Phase 3: dashboard, progress, activity, sessions ─────────────────────
router.get('/dashboard', studentsController.getDashboard);
router.get('/progress', studentsController.getProgress);
router.get('/activity', studentsController.getActivity);
router.get('/sessions', studentsController.getSessions);

export default router;
