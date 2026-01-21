import type { FastifyBaseLogger } from 'fastify';

import { getActiveFetchJob, startFetchJob, type FetchJob, type FetchMode } from './fetchRunner.js';

export interface ScheduledFetchConfig {
  enabled: boolean;
  intervalMinutes: number;
  term: string;
  campus: string;
  mode: FetchMode;
}

export interface ScheduledFetchState {
  config: ScheduledFetchConfig;
  status: 'idle' | 'scheduled' | 'running';
  lastRunAt: string | null;
  lastRunStatus: 'success' | 'error' | null;
  lastRunMessage: string | null;
  nextRunAt: string | null;
}

const MIN_INTERVAL_MINUTES = 1;
const MAX_INTERVAL_MINUTES = 30;
const DEFAULT_INTERVAL_MINUTES = 5;
const POLL_INTERVAL_MS = 5000; // Check job status every 5 seconds

export class ScheduledFetcher {
  private config: ScheduledFetchConfig;
  private timer: NodeJS.Timeout | null = null;
  private statusPollTimer: NodeJS.Timeout | null = null;
  private lastRunAt: string | null = null;
  private lastRunStatus: 'success' | 'error' | null = null;
  private lastRunMessage: string | null = null;
  private nextRunAt: string | null = null;
  private currentJob: FetchJob | null = null;
  private logger: FastifyBaseLogger | null = null;
  private sqliteFile: string;

  constructor(sqliteFile: string, logger?: FastifyBaseLogger) {
    this.sqliteFile = sqliteFile;
    this.logger = logger ?? null;
    this.config = {
      enabled: false,
      intervalMinutes: DEFAULT_INTERVAL_MINUTES,
      term: '',
      campus: '',
      mode: 'incremental',
    };
  }

  getState(): ScheduledFetchState {
    const activeJob = getActiveFetchJob();
    const isRunning = activeJob?.status === 'running' && this.currentJob?.id === activeJob.id;

    return {
      config: { ...this.config },
      status: isRunning ? 'running' : this.config.enabled ? 'scheduled' : 'idle',
      lastRunAt: this.lastRunAt,
      lastRunStatus: this.lastRunStatus,
      lastRunMessage: this.lastRunMessage,
      nextRunAt: this.nextRunAt,
    };
  }

  start(config: Partial<ScheduledFetchConfig>): ScheduledFetchState {
    this.updateConfig(config);
    this.config.enabled = true;

    if (!this.config.term || !this.config.campus) {
      throw new Error('Term and campus are required');
    }

    this.scheduleNextRun();
    this.logger?.info(
      { term: this.config.term, campus: this.config.campus, intervalMinutes: this.config.intervalMinutes },
      'Scheduled fetch started'
    );

    return this.getState();
  }

  stop(): ScheduledFetchState {
    this.config.enabled = false;
    this.clearTimers();
    this.nextRunAt = null;
    this.logger?.info('Scheduled fetch stopped');
    return this.getState();
  }

  updateConfig(partial: Partial<ScheduledFetchConfig>): void {
    if (partial.intervalMinutes !== undefined) {
      this.config.intervalMinutes = Math.max(
        MIN_INTERVAL_MINUTES,
        Math.min(MAX_INTERVAL_MINUTES, partial.intervalMinutes)
      );
    }
    if (partial.term !== undefined) {
      this.config.term = partial.term;
    }
    if (partial.campus !== undefined) {
      this.config.campus = partial.campus;
    }
    if (partial.mode !== undefined) {
      this.config.mode = partial.mode;
    }

    // Reschedule if enabled and config changed
    if (this.config.enabled && this.config.term && this.config.campus) {
      this.scheduleNextRun();
    }
  }

  private clearTimers(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    if (this.statusPollTimer !== null) {
      clearInterval(this.statusPollTimer);
      this.statusPollTimer = null;
    }
  }

  private scheduleNextRun(): void {
    this.clearTimers();

    if (!this.config.enabled) {
      return;
    }

    const delayMs = this.config.intervalMinutes * 60 * 1000;
    const nextTime = new Date(Date.now() + delayMs);
    this.nextRunAt = nextTime.toISOString();

    this.timer = setTimeout(() => {
      void this.executeRun();
    }, delayMs);

    this.logger?.debug(
      { nextRunAt: this.nextRunAt, intervalMinutes: this.config.intervalMinutes },
      'Scheduled next fetch run'
    );
  }

  private async executeRun(): Promise<void> {
    if (!this.config.enabled) {
      return;
    }

    // Check if a manual fetch is already running
    const activeJob = getActiveFetchJob();
    if (activeJob?.status === 'running') {
      this.logger?.info('Skipping scheduled fetch: manual fetch is running');
      this.scheduleNextRun();
      return;
    }

    this.logger?.info(
      { term: this.config.term, campus: this.config.campus },
      'Starting scheduled fetch'
    );

    try {
      this.currentJob = startFetchJob({
        term: this.config.term,
        campus: this.config.campus,
        mode: this.config.mode,
        sqliteFile: this.sqliteFile,
        logger: this.logger ?? undefined,
      });

      // Poll for job completion
      this.pollJobStatus();
    } catch (error) {
      this.lastRunAt = new Date().toISOString();
      this.lastRunStatus = 'error';
      this.lastRunMessage = (error as Error).message;
      this.logger?.error({ err: error }, 'Scheduled fetch failed to start');
      this.scheduleNextRun();
    }
  }

  private pollJobStatus(): void {
    if (this.statusPollTimer !== null) {
      clearInterval(this.statusPollTimer);
    }

    this.statusPollTimer = setInterval(() => {
      const job = getActiveFetchJob();

      if (!job || job.id !== this.currentJob?.id) {
        // Job completed or different job
        this.handleJobCompletion();
        return;
      }

      if (job.status !== 'running') {
        this.handleJobCompletion();
      }
    }, POLL_INTERVAL_MS);
  }

  private handleJobCompletion(): void {
    if (this.statusPollTimer !== null) {
      clearInterval(this.statusPollTimer);
      this.statusPollTimer = null;
    }

    const job = this.currentJob;
    if (job) {
      this.lastRunAt = job.finishedAt ?? new Date().toISOString();
      this.lastRunStatus = job.status === 'success' ? 'success' : 'error';
      this.lastRunMessage = job.status === 'error' ? (job.message ?? 'Unknown error') : null;

      this.logger?.info(
        { term: job.term, campus: job.campus, status: job.status },
        'Scheduled fetch completed'
      );
    }

    this.currentJob = null;

    // Schedule next run if still enabled
    if (this.config.enabled) {
      this.scheduleNextRun();
    }
  }

  destroy(): void {
    this.config.enabled = false;
    this.clearTimers();
    this.logger?.info('Scheduled fetcher destroyed');
  }
}
