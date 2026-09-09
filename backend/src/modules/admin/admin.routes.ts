import { Router } from 'express';
import * as adminController from './admin.controller';
import { authenticate } from '../../middleware/authenticate';
import { requireRole } from '../../middleware/authorize';
import { validate } from '../../middleware/validate';
import { listStudentsQuerySchema, updateStudentAcademicSchema, studentStatusSchema } from '../../validators/student.validator';

const router = Router();

// All admin routes require authentication + ADMIN role
// (a STUDENT token here → 403 from requireRole; suspended/inactive → 403 from authenticate)
router.use(authenticate, requireRole('ADMIN'));

// Dashboard
router.get('/me', adminController.getMe);
router.get('/dashboard/stats', adminController.dashboardStats);

// Students management
router.get('/students', validate(listStudentsQuerySchema, 'query'), adminController.listStudents);
router.get('/students/:id', adminController.getStudentById);
router.patch('/students/:id', validate(updateStudentAcademicSchema), adminController.updateStudent);
router.patch('/students/:id/status', validate(studentStatusSchema), adminController.setStudentStatus);

// Reference data (admin view — includes inactive records)
router.get('/programs', adminController.listPrograms);
router.get('/semesters', adminController.listSemesters);
router.get('/lscs', adminController.listLscs);
router.get('/courses', adminController.listCourses);

export default router;
