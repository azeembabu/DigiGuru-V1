import { Role } from '@prisma/client';

declare global {
  namespace Express {
    interface Request {
      user?: {
        id: string;
        email: string;
        role: Role;
        /** Session this access token belongs to; absent on legacy tokens. */
        sessionId?: string;
      };
    }
  }
}

export {};
