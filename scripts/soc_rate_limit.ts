import { writeFileSync } from 'node:fs';

import {
  decodeSemester,
  performProbe,
  type Endpoint,
  type SemesterParts,
  SOCRequestError
} from './soc_api_client.js';

interface ScenarioSpec {
  concurrency: number;
  intervalMs: number;
}

interface RateLimitCLIOptions {
  term: string;
  campus: string;
  subject?: string;
  level?: string;
  endpoint: Endpoint | 'both';
  schedule: ScenarioSpec[];
  iterations: number;
  timeoutMs: number;
  restMs: number;
  outputPath?: string;
  label?: string;
}

type StatusBucket = '2xx' | '4xx' | '5xx' | 'timeout' | 'network' | 'json' | 'other';

interface StatusCounts extends Record<StatusBucket, number> {}

interface ErrorSample {
  endpoint: Endpoint;
  requestId: string;
  kind: string;
  statusCode?: number;
  statusText?: string;
  retryHint?: string;
  detail?: string;
}

interface ScenarioResult {
  endpoint: Endpoint;
  concurrency: number;
  intervalMs: number;
  iterations: number;
  estimatedRps: number;
  actualRps: number;
  durationMs: number;
  avgLatencyMs: number;
  p95LatencyMs: number;
  statusCounts: StatusCounts;
  errorSamples: ErrorSample[];
}

class CLIError extends Error {}

async function main(): Promise<void> {
  try {
    const options = parseArgs();
    const semester = decodeSemesterSafe(options.term);
    const endpoints = options.endpoint === 'both' ? (['courses', 'openSections'] as Endpoint[]) : [options.endpoint];

    const results: ScenarioResult[] = [];
    for (const endpoint of endpoints) {
      for (const scenario of options.schedule) {
        console.log(`\n== ${endpoint} | concurrency=${scenario.concurrency} | interval=${scenario.intervalMs}ms ==`);
        const scenarioResult = await runScenario(
          {
            campus: options.campus,
            subject: options.subject,
            level: options.level,
            endpoint,
            timeoutMs: options.timeoutMs
          },
          semester,
          scenario,
          options.iterations
        );
        results.push(scenarioResult);
        formatScenarioResult(scenarioResult);
        if (options.restMs > 0) {
          console.log(`Cooling down for ${options.restMs} ms before next scenario...`);
          await delay(options.restMs);
        }
      }
    }

    if (options.outputPath) {
      const payload = {
        generatedAt: new Date().toISOString(),
        label: options.label ?? null,
        term: options.term,
        campus: options.campus,
        subject: options.subject ?? null,
        level: options.level ?? null,
        iterationsPerScenario: options.iterations,
        schedule: options.schedule,
        results
      };
      writeFileSync(options.outputPath, JSON.stringify(payload, null, 2), 'utf-8');
      console.log(`\nSaved raw results to ${options.outputPath}`);
    }
  } catch (error) {
    if (error instanceof CLIError) {
      console.error(`Argument error: ${error.message}`);
    } else if (error instanceof Error) {
      console.error(`Rate-limit runner failed: ${error.message}`);
    } else {
      console.error('Unknown failure in rate-limit runner.');
    }
    process.exit(1);
  }
}

function parseArgs(): RateLimitCLIOptions {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    showUsage();
    throw new CLIError('Missing arguments.');
  }

  const opts: Partial<RateLimitCLIOptions> = {
    endpoint: 'both',
    iterations: 24,
    timeoutMs: 15000,
    restMs: 2000,
    schedule: parseSchedule('1:1000,3:600,6:300')
  };

  for (let i = 0; i < args.length; i += 1) {
    const token = args[i];
    if (token === '--help' || token === '-h') {
      showUsage();
      process.exit(0);
    }
    if (!token.startsWith('--')) {
      throw new CLIError(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    const value = args[i + 1];
    if (value === undefined || value.startsWith('--')) {
      throw new CLIError(`Missing value for --${key}`);
    }

    switch (key) {
      case 'term':
        opts.term = value;
        break;
      case 'campus':
        opts.campus = value.toUpperCase();
        break;
      case 'subject':
        opts.subject = value.toUpperCase();
        break;
      case 'level':
        opts.level = value.toUpperCase();
        break;
      case 'endpoint': {
        const normalized = value.toLowerCase();
        if (normalized === 'both') {
          opts.endpoint = 'both';
        } else if (normalized === 'courses' || normalized === 'opensections') {
          opts.endpoint = normalized === 'courses' ? 'courses' : 'openSections';
        } else {
          throw new CLIError('endpoint must be courses, openSections, or both');
        }
        break;
      }
      case 'schedule':
        opts.schedule = parseSchedule(value);
        break;
      case 'iterations': {
        const parsed = Number.parseInt(value, 10);
        if (Number.isNaN(parsed) || parsed <= 0) {
          throw new CLIError('iterations must be a positive integer');
        }
        opts.iterations = parsed;
        break;
      }
      case 'timeout': {
        const parsed = Number.parseInt(value, 10);
        if (Number.isNaN(parsed) || parsed <= 0) {
          throw new CLIError('timeout must be a positive integer');
        }
        opts.timeoutMs = parsed;
        break;
      }
      case 'rest': {
        const parsed = Number.parseInt(value, 10);
        if (Number.isNaN(parsed) || parsed < 0) {
          throw new CLIError('rest must be a non-negative integer');
        }
        opts.restMs = parsed;
        break;
      }
      case 'output':
        opts.outputPath = value;
        break;
      case 'label':
        opts.label = value;
        break;
      default:
        throw new CLIError(`Unknown flag: --${key}`);
    }
    i += 1;
  }

  if (!opts.term) {
    throw new CLIError('Missing required --term');
  }
  if (!opts.campus) {
    throw new CLIError('Missing required --campus');
  }

  return opts as RateLimitCLIOptions;
}

function showUsage(): void {
  console.log(`Rutgers SOC rate-limit profiler
Usage: npm run soc:rate-limit -- --term <semester> --campus <code> [flags]

Examples:
  npm run soc:rate-limit -- --term 12024 --campus NB --endpoint both
  npm run soc:rate-limit -- --term 92024 --campus NK --endpoint courses --schedule 1:1200,2:600,4:400 --iterations 20

Flags:
  --subject       Optional subject code (courses endpoint only; still logged for context)
  --level         Optional level hint forwarded to the API
  --endpoint      courses | openSections | both (default: both)
  --schedule      Comma list of concurrency:intervalMs (default: 1:1000,3:600,6:300)
  --iterations    Requests per scenario (default: 24)
  --timeout       Request timeout in ms (default: 15000)
  --rest          Cooldown between scenarios in ms (default: 2000)
  --output        Optional path to save raw JSON results
  --label         Optional note written into the JSON payload
`);
}

function parseSchedule(value: string): ScenarioSpec[] {
  const pairs = value.split(',');
  if (pairs.length === 0) {
    throw new CLIError('schedule must contain at least one concurrency:interval pair');
  }
  const schedule = pairs.map((pair) => {
    const [concurrencyRaw, intervalRaw] = pair.split(':');
    const concurrency = Number.parseInt(concurrencyRaw, 10);
    const intervalMs = Number.parseInt(intervalRaw, 10);
    if (Number.isNaN(concurrency) || concurrency <= 0) {
      throw new CLIError(`Invalid concurrency in schedule pair "${pair}"`);
    }
    if (Number.isNaN(intervalMs) || intervalMs < 0) {
      throw new CLIError(`Invalid interval in schedule pair "${pair}"`);
    }
    return { concurrency, intervalMs };
  });
  return schedule;
}

function decodeSemesterSafe(term: string): SemesterParts {
  try {
    return decodeSemester(term);
  } catch (error) {
    throw new CLIError((error as Error).message);
  }
}

async function runScenario(
  requestOptions: {
    campus: string;
    subject?: string;
    level?: string;
    endpoint: Endpoint;
    timeoutMs: number;
  },
  semester: SemesterParts,
  scenario: ScenarioSpec,
  iterations: number
): Promise<ScenarioResult> {
  const statusCounts: StatusCounts = {
    '2xx': 0,
    '4xx': 0,
    '5xx': 0,
    timeout: 0,
    network: 0,
    json: 0,
    other: 0
  };
  const durations: number[] = [];
  const errorSamples: ErrorSample[] = [];
  let issued = 0;

  const worker = async (): Promise<void> => {
    while (true) {
      const current = issued;
      issued += 1;
      if (current >= iterations) {
        break;
      }
      try {
        const result = await performProbe(requestOptions, semester);
        durations.push(result.durationMs);
        incrementStatusCount(statusCounts, result.statusCode);
      } catch (error) {
        if (error instanceof SOCRequestError) {
          incrementErrorCount(statusCounts, error);
          if (errorSamples.length < 5) {
            errorSamples.push({
              endpoint: error.endpoint,
              requestId: error.requestId,
              kind: error.kind,
              statusCode: error.statusCode,
              statusText: error.statusText,
              retryHint: error.retryHint,
              detail: error.detail
            });
          }
        } else {
          statusCounts.other += 1;
          if (errorSamples.length < 5) {
            errorSamples.push({
              endpoint: requestOptions.endpoint,
              requestId: 'unknown',
              kind: 'UNKNOWN',
              detail: (error as Error).message
            });
          }
        }
      }
      if (scenario.intervalMs > 0) {
        await delay(scenario.intervalMs);
      }
    }
  };

  const started = Date.now();
  const workers = Array.from({ length: scenario.concurrency }, () => worker());
  await Promise.all(workers);
  const runDuration = Date.now() - started;

  const actualRps = Number(
    (runDuration === 0 ? iterations : iterations / (runDuration / 1000)).toFixed(2)
  );
  const estimatedRps = scenario.intervalMs === 0 ? actualRps : estimateRps(scenario);

  const avgLatency = durations.length === 0 ? 0 : mean(durations);
  const p95Latency = durations.length === 0 ? 0 : percentile(durations, 0.95);

  return {
    endpoint: requestOptions.endpoint,
    concurrency: scenario.concurrency,
    intervalMs: scenario.intervalMs,
    iterations,
    estimatedRps,
    actualRps,
    durationMs: runDuration,
    avgLatencyMs: avgLatency,
    p95LatencyMs: p95Latency,
    statusCounts,
    errorSamples
  };
}

function incrementStatusCount(counts: StatusCounts, statusCode: number): void {
  if (statusCode >= 200 && statusCode < 300) {
    counts['2xx'] += 1;
  } else if (statusCode >= 400 && statusCode < 500) {
    counts['4xx'] += 1;
  } else if (statusCode >= 500 && statusCode < 600) {
    counts['5xx'] += 1;
  } else {
    counts.other += 1;
  }
}

function incrementErrorCount(counts: StatusCounts, error: SOCRequestError): void {
  if (error.kind === 'HTTP') {
    if (error.statusCode && error.statusCode >= 500) {
      counts['5xx'] += 1;
    } else if (error.statusCode && error.statusCode >= 400) {
      counts['4xx'] += 1;
    } else {
      counts.other += 1;
    }
    return;
  }
  if (error.kind === 'TIMEOUT') {
    counts.timeout += 1;
  } else if (error.kind === 'NETWORK') {
    counts.network += 1;
  } else if (error.kind === 'JSON_PARSE') {
    counts.json += 1;
  } else {
    counts.other += 1;
  }
}

function estimateRps(scenario: ScenarioSpec): number {
  const interval = Math.max(scenario.intervalMs, 1);
  return Number((scenario.concurrency * (1000 / interval)).toFixed(2));
}

function formatScenarioResult(result: ScenarioResult): void {
  console.log(
    `Requests=${result.iterations} • Estimated RPS=${result.estimatedRps} • Actual RPS=${result.actualRps} • Run duration=${result.durationMs} ms`
  );
  console.log(
    `Latency avg=${result.avgLatencyMs.toFixed(1)} ms • p95=${result.p95LatencyMs.toFixed(1)} ms`
  );
  console.log(
    `Status counts → 2xx:${result.statusCounts['2xx']} 4xx:${result.statusCounts['4xx']} 5xx:${result.statusCounts['5xx']} timeout:${result.statusCounts.timeout} network:${result.statusCounts.network} json:${result.statusCounts.json}`
  );
  if (result.errorSamples.length > 0) {
    console.log('Sample errors:');
    result.errorSamples.forEach((sample) => {
      const status = sample.statusCode ? ` status=${sample.statusCode}` : '';
      const hint = sample.retryHint ? ` hint=${sample.retryHint}` : '';
      console.log(`  • [${sample.requestId}] kind=${sample.kind}${status}${hint}`);
    });
  } else {
    console.log('No errors captured in this scenario.');
  }
}

function mean(values: number[]): number {
  const sum = values.reduce((acc, value) => acc + value, 0);
  return sum / values.length;
}

function percentile(values: number[], percentileRank: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.min(sorted.length - 1, Math.max(0, Math.floor(percentileRank * sorted.length)));
  return sorted[idx];
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main();
