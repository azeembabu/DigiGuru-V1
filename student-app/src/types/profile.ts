/**
 * Shapes returned by the profile and session endpoints.
 *
 * Kept in one place so the page, its section components and the data hooks all
 * agree on the contract. These mirror the backend responses exactly — no
 * tokens, hashes or internal ids ever cross this boundary.
 */

export interface ProgramRef {
  name: string;
  code: string;
}

export interface SemesterRef {
  name: string;
  semesterNumber: number;
}

export interface LscRef {
  name: string;
  code: string;
}

export type AccountStatus = 'ACTIVE' | 'INACTIVE' | 'SUSPENDED';

/** GET / PATCH /api/student/profile */
export interface StudentProfile {
  fullName: string;
  rollNumber: string;
  email: string;
  phoneNumber: string;
  program: ProgramRef | null;
  semester: SemesterRef | null;
  lsc: LscRef | null;
  accountStatus: AccountStatus;
  accountCreatedAt: string;
  lastLoginAt: string | null;
  /**
   * Authoritative list of fields the backend will accept on PATCH. The UI reads
   * this instead of assuming editability from the mere presence of a field.
   */
  editableFields: string[];
}

/** Fields a student may send to PATCH /api/student/profile. */
export interface ProfileUpdate {
  fullName?: string;
  phoneNumber?: string;
}

/** One active session from GET /api/auth/sessions. */
export interface ActiveSession {
  id: string;
  device: string;
  ipAddress: string | null;
  createdAt: string;
  expiresAt: string;
  /** Remember Me was chosen — a long-lived (30d) rather than 8h session. */
  remembered: boolean;
  isCurrent: boolean;
}
