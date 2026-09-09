import type { Request, Response, NextFunction } from 'express';
import type { ZodTypeAny, z } from 'zod';

type Target = 'body' | 'query';

/**
 * Zod validation middleware. On failure responds 400 with the standard error
 * envelope; on success replaces the target (`body` by default, or `query`)
 * with the parsed (defaulted, coerced, trimmed) data.
 */
export function validate<T extends ZodTypeAny>(schema: T, target: Target = 'body') {
  return (req: Request, res: Response, next: NextFunction): void => {
    const result = schema.safeParse(req[target]);
    if (!result.success) {
      res.status(400).json({
        success: false,
        error: 'Validation failed',
        details: result.error.errors.map((e) => ({ path: e.path.join('.'), message: e.message })),
      });
      return;
    }
    if (target === 'body') {
      req.body = result.data as z.infer<T>;
    } else {
      // Express 4's req.query getter is read-only; mutate the underlying object.
      Object.assign(req.query, result.data);
    }
    next();
  };
}

export default validate;
