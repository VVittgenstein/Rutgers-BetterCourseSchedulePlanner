import { z } from 'zod';

export const API_VERSION = 'v1';

export const optionalBooleanParam = z
  .union([z.boolean(), z.enum(['true', 'false', '1', '0'])])
  .optional()
  .transform((value) => {
    if (value === undefined) {
      return undefined;
    }
    if (typeof value === 'boolean') {
      return value;
    }
    return value === 'true' || value === '1';
  });

export const minutesParam = z.coerce.number().int().min(0).max(1440);

export const optionalMinutesParam = minutesParam.optional();

export const stringOrArrayParam = z
  .union([z.string(), z.array(z.string())])
  .optional()
  .transform((value) => {
    if (!value) {
      return undefined;
    }
    const values = Array.isArray(value)
      ? value
      : value
          .split(',')
          .map((token) => token.trim())
          .filter(Boolean);
    return values.length > 0 ? values : undefined;
  });

export function enumArrayParam<const T extends [string, ...string[]]>(values: T) {
  const base = z.enum(values);
  return z
    .union([base, base.array()])
    .optional()
    .transform((value) => {
      if (!value) {
        return undefined;
      }
      return Array.isArray(value) ? (value.length ? value : undefined) : [value];
    });
}

export function paginationSchema(maxPageSize: number, defaultPageSize: number) {
  return z.object({
    page: z.coerce.number().int().min(1).default(1),
    pageSize: z.coerce.number().int().min(1).max(maxPageSize).default(defaultPageSize),
  });
}

export const sortDirectionSchema = z.enum(['asc', 'desc']).default('asc');
