import { randomUUID } from 'node:crypto';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import type { FastifyBaseLogger } from 'fastify';

export type FetchMode = 'full-init' | 'incremental';
export type FetchJobStatus = 'running' | 'success' | 'error';

export interface FetchJob {
  id: string;
  term: string;
  campus: string;
  mode: FetchMode;
  status: FetchJobStatus;
  startedAt: string;
  finishedAt: string | null;
  message?: string;
  logFile?: string;
}

export interface StartFetchJobOptions {
  term: string;
  campus: string;
  mode: FetchMode;
  sqliteFile: string;
  baseConfigPath?: string;
  workdir?: string;
  logger?: FastifyBaseLogger;
}

const DEFAULT_BASE_CONFIG = path.resolve('configs', 'fetch_pipeline.local.json');
const DEFAULT_BASE_CONFIG_FALLBACK = path.resolve('configs', 'fetch_pipeline.example.json');
const RUNTIME_DIR = path.resolve('data', 'runtime');
const DEFAULT_LOG_DIR = path.resolve('logs', 'fetch_runs');

let activeJob: FetchJob | null = null;
let activeChild: ChildProcessWithoutNullStreams | null = null;

function ensureDir(dir: string) {
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function resolveBaseConfig(customPath?: string) {
  if (customPath && fs.existsSync(customPath)) {
    return customPath;
  }
  if (fs.existsSync(DEFAULT_BASE_CONFIG)) {
    return DEFAULT_BASE_CONFIG;
  }
  if (fs.existsSync(DEFAULT_BASE_CONFIG_FALLBACK)) {
    return DEFAULT_BASE_CONFIG_FALLBACK;
  }
  throw new Error('Missing fetch pipeline config under configs/.');
}

function readJsonSafe(file: string): Record<string, unknown> {
  try {
    const raw = fs.readFileSync(file, 'utf8');
    return JSON.parse(raw) as Record<string, unknown>;
  } catch (error) {
    throw new Error(`Unable to read ${file}: ${(error as Error).message}`);
  }
}

function resolvePath(input: string) {
  if (path.isAbsolute(input)) return input;
  return path.resolve(input);
}

function resolveNpmLike(base: string) {
  if (process.platform !== 'win32') return base;
  const candidate = `${base}.cmd`;
  return candidate;
}

function buildRuntimeConfig(options: StartFetchJobOptions, job: FetchJob) {
  ensureDir(RUNTIME_DIR);
  const baseConfigPath = resolveBaseConfig(options.baseConfigPath);
  const baseConfig = readJsonSafe(baseConfigPath);

  const sqliteFile =
    options.sqliteFile && options.sqliteFile.length > 0
      ? resolvePath(options.sqliteFile)
      : typeof baseConfig.sqliteFile === 'string' && baseConfig.sqliteFile.length > 0
        ? resolvePath(baseConfig.sqliteFile)
        : resolvePath(path.resolve('data', 'local.db'));

  const mergedConfig = {
    ...baseConfig,
    runLabel: (baseConfig.runLabel as string) ?? 'webui',
    defaultMode: (baseConfig.defaultMode as FetchMode | undefined) ?? options.mode,
    sqliteFile,
    targets: [
      {
        term: options.term,
        mode: options.mode,
        campuses: [{ code: options.campus, subjects: ['ALL'] }],
      },
    ],
  };

  const logDirRaw = (baseConfig.logDir as string | undefined) ?? DEFAULT_LOG_DIR;
  const logDir = ensureDir(resolvePath(logDirRaw));
  const stagingDirRaw = (baseConfig.stagingDir as string | undefined) ?? path.resolve('data', 'staging');
  ensureDir(resolvePath(stagingDirRaw));

  const configPath = path.join(RUNTIME_DIR, `fetch_job_${job.id}.json`);
  fs.writeFileSync(configPath, JSON.stringify(mergedConfig, null, 2));

  return { configPath, logDir, sqliteFile };
}

function cleanupRuntimeConfig(configPath: string, logger?: FastifyBaseLogger) {
  try {
    fs.rmSync(configPath);
  } catch (error) {
    logger?.warn({ err: error }, `Failed to clean up temp fetch config ${configPath}`);
  }
}

export function getActiveFetchJob() {
  return activeJob;
}

export function startFetchJob(options: StartFetchJobOptions): FetchJob {
  if (activeJob && activeJob.status === 'running') {
    throw new Error('Fetch is already running');
  }

  const startedAt = new Date().toISOString();
  const job: FetchJob = {
    id: randomUUID(),
    term: options.term,
    campus: options.campus,
    mode: options.mode,
    status: 'running',
    startedAt,
    finishedAt: null,
  };

  activeJob = job;
  options.logger?.info({ term: job.term, campus: job.campus, mode: job.mode }, 'Starting webui fetch job');

  const { configPath, logDir } = buildRuntimeConfig(options, job);
  const logFile = path.join(logDir, `webui_fetch_${job.id}.log`);
  job.logFile = path.relative(process.cwd(), logFile);

  const logStream = fs.createWriteStream(logFile, { flags: 'a' });
  logStream.write(`[webui] Starting fetch for term=${job.term}, campus=${job.campus}, mode=${job.mode}\n`);

  const npmCmd = resolveNpmLike('npm');
  let child: ChildProcessWithoutNullStreams;
  try {
    child = spawn(
      npmCmd,
      ['run', 'data:fetch', '--', '--config', configPath, '--mode', options.mode, '--terms', options.term, '--campuses', options.campus],
      {
        cwd: options.workdir ?? process.cwd(),
        env: { ...process.env, SQLITE_FILE: options.sqliteFile },
        stdio: ['ignore', 'pipe', 'pipe'],
        shell: process.platform === 'win32',
      },
    );
  } catch (error) {
    job.finishedAt = new Date().toISOString();
    job.status = 'error';
    job.message = `Failed to start fetch: ${(error as Error).message}`;
    logStream.write(`[webui] Failed to start fetch: ${(error as Error).message}\n`);
    logStream.end();
    cleanupRuntimeConfig(configPath, options.logger);
    throw error;
  }
  activeChild = child;

  child.stdout?.pipe(logStream, { end: false });
  child.stderr?.pipe(logStream, { end: false });

  child.on('exit', (code, signal) => {
    job.finishedAt = new Date().toISOString();
    activeChild = null;
    logStream.write(`[webui] Fetch process exited with code=${code ?? 'unknown'} signal=${signal ?? 'none'}\n`);
    if (typeof code === 'number' && code === 0) {
      job.status = 'success';
      options.logger?.info({ term: job.term, campus: job.campus }, 'Webui fetch job completed');
    } else {
      job.status = 'error';
      job.message =
        typeof code === 'number'
          ? `Fetch exited with code ${code}`
          : signal
            ? `Fetch terminated by signal ${signal}`
            : 'Fetch process ended unexpectedly';
      options.logger?.error({ term: job.term, campus: job.campus, message: job.message }, 'Webui fetch job failed');
    }
    logStream.end();
    cleanupRuntimeConfig(configPath, options.logger);
  });

  child.on('error', (error) => {
    job.finishedAt = new Date().toISOString();
    job.status = 'error';
    job.message = `Failed to start fetch: ${(error as Error).message}`;
    activeChild = null;
    logStream.write(`[webui] Failed to start fetch: ${(error as Error).message}\n`);
    logStream.end();
    cleanupRuntimeConfig(configPath, options.logger);
    options.logger?.error({ term: job.term, campus: job.campus, message: job.message }, 'Webui fetch process failed to start');
  });

  return job;
}
