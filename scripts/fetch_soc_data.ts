#!/usr/bin/env tsx
import { readFile } from 'node:fs/promises';
import path from 'node:path';

type PipelineMode = 'full-init' | 'incremental';

interface CLIOptions {
  configPath?: string;
  modeOverride?: PipelineMode;
  termFilter?: string[];
  campusFilter?: string[];
  subjectFilter?: string[];
  subjectAllOverride: boolean;
  maxWorkers?: number;
  resumePath?: string;
  dryRun: boolean;
  showHelp: boolean;
}

interface PipelineConfig {
  runLabel?: string;
  defaultMode?: PipelineMode;
  sqliteFile?: string;
  stagingDir?: string;
  logDir?: string;
  rateLimitProfile?: string;
  concurrency?: Record<string, unknown>;
  retryPolicy?: Record<string, unknown>;
  targets?: PipelineTarget[];
  incremental?: Record<string, unknown>;
  fullInit?: Record<string, unknown>;
  summary?: Record<string, unknown>;
  safety?: Record<string, unknown>;
}

interface PipelineTarget {
  term: string;
  mode?: PipelineMode;
  campuses?: CampusConfig[];
  subjectBatchSize?: number;
  subjectRecencyMinutes?: number;
}

interface CampusConfig {
  code: string;
  subjects?: string[];
}

interface PlannedSlice {
  term: string;
  campus: string;
  mode: PipelineMode;
  subjects: string[];
}

class CLIError extends Error {}

function parseArgs(argv: string[]): CLIOptions {
  const options: CLIOptions = {
    subjectAllOverride: false,
    dryRun: false,
    showHelp: false
  };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) {
      throw new CLIError(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    switch (key) {
      case 'config':
        options.configPath = requireValue(argv, ++i, '--config');
        break;
      case 'mode':
        options.modeOverride = normalizeMode(requireValue(argv, ++i, '--mode'));
        break;
      case 'terms':
        options.termFilter = parseList(requireValue(argv, ++i, '--terms'));
        break;
      case 'campuses':
        options.campusFilter = parseList(requireValue(argv, ++i, '--campuses')).map((item) =>
          item.toUpperCase(),
        );
        break;
      case 'subjects': {
        const subjects = parseList(requireValue(argv, ++i, '--subjects')).map((subject) =>
          subject.toUpperCase(),
        );
        if (subjects.includes('ALL')) {
          options.subjectAllOverride = true;
        } else if (subjects.length > 0) {
          options.subjectFilter = subjects;
        }
        break;
      }
      case 'max-workers': {
        const raw = requireValue(argv, ++i, '--max-workers');
        const parsed = Number.parseInt(raw, 10);
        if (Number.isNaN(parsed) || parsed <= 0) {
          throw new CLIError('--max-workers must be a positive integer');
        }
        options.maxWorkers = parsed;
        break;
      }
      case 'resume':
        options.resumePath = requireValue(argv, ++i, '--resume');
        break;
      case 'dry-run':
        options.dryRun = true;
        break;
      case 'help':
        options.showHelp = true;
        break;
      default:
        throw new CLIError(`Unknown flag: --${key}`);
    }
  }

  return options;
}

function requireValue(argv: string[], index: number, flag: string): string {
  const value = argv[index];
  if (!value) {
    throw new CLIError(`Missing value for ${flag}`);
  }
  return value;
}

function normalizeMode(value: string): PipelineMode {
  const normalized = value.toLowerCase();
  if (normalized === 'full-init' || normalized === 'incremental') {
    return normalized;
  }
  throw new CLIError(`Invalid mode: ${value}`);
}

function parseList(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function showUsage(): void {
  console.log(`SOC Fetch pipeline (planning stub)

Usage:
  npm run data:fetch -- --config path/to/config.json [options]

Options:
  --config <path>      Required. Pipeline config JSON.
  --mode <name>        Override mode (full-init | incremental).
  --terms <list>       Comma list of term IDs to limit work.
  --campuses <list>    Comma list of campus codes (NB,NK,...).
  --subjects <list>    Subject override. Use ALL to process every subject.
  --max-workers <n>    Caps worker pools for quick experiments.
  --resume <path>      Resume queue override.
  --dry-run            Plan only. (Current behavior is plan-only regardless.)
  --help               Show this message.
`);
}

async function loadPipelineConfig(configPath: string): Promise<PipelineConfig> {
  const resolved = path.resolve(configPath);
  const contents = await readFile(resolved, 'utf8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(contents);
  } catch (error) {
    throw new CLIError(`Unable to parse config JSON (${(error as Error).message})`);
  }
  if (parsed === null || typeof parsed !== 'object') {
    throw new CLIError('Config must be a JSON object.');
  }
  const pipeline = parsed as PipelineConfig;
  if (!Array.isArray(pipeline.targets) || pipeline.targets.length === 0) {
    throw new CLIError('Config must include at least one entry in targets[].');
  }
  return pipeline;
}

function buildPlan(config: PipelineConfig, options: CLIOptions): PlannedSlice[] {
  const defaultMode: PipelineMode = normalizeMode(config.defaultMode ?? 'incremental');
  const termFilter = options.termFilter ? new Set(options.termFilter) : null;
  const campusFilter = options.campusFilter ? new Set(options.campusFilter) : null;
  const plan: PlannedSlice[] = [];

  for (const target of config.targets ?? []) {
    if (!target || typeof target !== 'object') continue;
    if (!target.term) {
      throw new CLIError('Each target must include a term field.');
    }
    if (termFilter && !termFilter.has(target.term)) continue;
    const mode = options.modeOverride ?? target.mode ?? defaultMode;
    const campuses = target.campuses ?? [];
    if (campuses.length === 0) {
      throw new CLIError(`Target ${target.term} must specify at least one campus.`);
    }
    for (const campus of campuses) {
      if (!campus.code) {
        throw new CLIError(`Campus entry under term ${target.term} is missing code.`);
      }
      const campusCode = campus.code.toUpperCase();
      if (campusFilter && !campusFilter.has(campusCode)) continue;
      const subjects = resolveSubjects(campus.subjects, options);
      plan.push({
        term: target.term,
        campus: campusCode,
        mode,
        subjects
      });
    }
  }

  if (plan.length === 0) {
    throw new CLIError('No work slices match the provided filters.');
  }

  return plan;
}

function resolveSubjects(subjects: string[] | undefined, options: CLIOptions): string[] {
  if (options.subjectAllOverride) {
    return ['ALL'];
  }
  if (options.subjectFilter && options.subjectFilter.length > 0) {
    return options.subjectFilter;
  }
  if (subjects && subjects.length > 0) {
    return subjects;
  }
  return ['ALL'];
}

function printPlan(plan: PlannedSlice[], config: PipelineConfig, options: CLIOptions): void {
  console.log('SOC fetch pipeline planner');
  if (config.runLabel) {
    console.log(`Run label: ${config.runLabel}`);
  }
  console.log(`SQLite file: ${config.sqliteFile ?? 'n/a'}`);
  console.log(`Staging dir: ${config.stagingDir ?? 'n/a'}`);
  console.log(`Log dir: ${config.logDir ?? 'n/a'}`);
  if (options.maxWorkers) {
    console.log(`Worker cap override: ${options.maxWorkers}`);
  }
  if (options.resumePath) {
    console.log(`Resume queue override: ${options.resumePath}`);
  }
  console.log('');
  console.log('Planned slices:');
  plan.forEach((slice, index) => {
    console.log(
      `  ${index + 1}. term=${slice.term} campus=${slice.campus} mode=${slice.mode} subjects=${slice.subjects.join(
        ',',
      )}`,
    );
  });
  console.log('');
  console.log('NOTE: Execution engine not implemented yet (see ST-20251113-act-001-02-ingest-impl).');
  if (options.dryRun) {
    console.log('Dry-run requested: this command currently performs planning only.');
  } else {
    console.log('This stub currently performs planning only; no network or database work was attempted.');
  }
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  if (options.showHelp) {
    showUsage();
    return;
  }
  if (!options.configPath) {
    throw new CLIError('Missing required --config <path>');
  }
  const config = await loadPipelineConfig(options.configPath);
  const plan = buildPlan(config, options);
  printPlan(plan, config, options);
}

void (async () => {
  try {
    await main();
  } catch (error) {
    if (error instanceof CLIError) {
      console.error(`Error: ${error.message}`);
      console.error('Use --help to see available options.');
      process.exit(1);
    } else {
      console.error('Unexpected error:', error);
      process.exit(1);
    }
  }
})();

