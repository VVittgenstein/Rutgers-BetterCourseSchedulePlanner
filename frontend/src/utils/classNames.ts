export type ClassValue = string | false | null | undefined;

export const classNames = (...values: ClassValue[]): string =>
  values.filter((value): value is string => typeof value === 'string' && value.length > 0).join(' ');
