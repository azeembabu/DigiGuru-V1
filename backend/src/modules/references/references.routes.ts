import { Router } from 'express';
import * as referencesController from './references.controller';

const router = Router();

// Public, read-only dropdown data for the signup form (see controller docblock).
router.get('/programs', referencesController.listPrograms);
router.get('/semesters', referencesController.listSemesters);
router.get('/lscs', referencesController.listLscs);

export default router;
