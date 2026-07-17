export type TermSeason = 'winter' | 'spring' | 'summer' | 'fall';

const SEASON_CODES: Record<TermSeason, string> = {
  winter: '0',
  spring: '1',
  summer: '7',
  fall: '9',
};

const TERM_ALIASES: Record<string, TermSeason> = {
  W: 'winter',
  WI: 'winter',
  WINTER: 'winter',
  S: 'spring',
  SP: 'spring',
  SPR: 'spring',
  SPRING: 'spring',
  SU: 'summer',
  SUM: 'summer',
  SUMMER: 'summer',
  F: 'fall',
  FA: 'fall',
  FALL: 'fall',
};

const TERM_CODES: Record<string, TermSeason> = {
  '0': 'winter',
  '1': 'spring',
  '7': 'summer',
  '9': 'fall',
};

export function buildTermId(year: number, season: TermSeason): string {
  const normalizedYear = Math.max(2000, Math.min(2100, Math.floor(year)));
  return `${SEASON_CODES[season]}${normalizedYear}`;
}

export function parseTermId(raw: string): { year: number | null; season: TermSeason | null } {
  if (!raw) return { year: null, season: null };
  const stripped = raw.replace(/[-_\s]/g, '').toUpperCase();

  const direct = stripped.match(/^([0179])(\d{4})$/);
  if (direct) {
    const [, code, year] = direct;
    return { year: Number.parseInt(year, 10), season: TERM_CODES[code] ?? null };
  }

  const swapped = stripped.match(/^(\d{4})([0179])$/);
  if (swapped) {
    const [, year, code] = swapped;
    return { year: Number.parseInt(year, 10), season: TERM_CODES[code] ?? null };
  }

  const alias = stripped.match(/^([A-Z]+)(\d{4})$/);
  if (alias) {
    const [, aliasCode, year] = alias;
    const season = TERM_ALIASES[aliasCode];
    if (season) {
      return { year: Number.parseInt(year, 10), season };
    }
  }

  return { year: null, season: null };
}

export function seasonLabelKey(season: TermSeason) {
  switch (season) {
    case 'spring':
      return 'spring';
    case 'summer':
      return 'summer';
    case 'fall':
      return 'fall';
    case 'winter':
    default:
      return 'winter';
  }
}
