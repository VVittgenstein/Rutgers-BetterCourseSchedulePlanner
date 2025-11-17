import {
  decodeSemester,
  ENDPOINTS,
  performProbe,
  type Endpoint,
  type ProbeResult,
  type SemesterParts,
  SOCRequestError
} from './soc_api_client.js';

interface CLIOptions {
  term: string;
  campus: string;
  subject?: string;
  level?: string;
  endpoint: Endpoint;
  sampleSize: number;
  timeoutMs: number;
}

interface CourseRecord {
  subject?: string;
  courseNumber?: string;
  title?: string;
  campusLocations?: Array<{ description?: string }>;
  sections?: Array<unknown>;
  openSections?: number;
  credits?: string | number;
}

interface CourseSummary {
  totalRecords: number;
  filteredRecords: number;
  sample: Array<Record<string, unknown>>;
  note?: string;
}

interface OpenSectionsSummary {
  totalRecords: number;
  sample: string[];
}

class CLIError extends Error {}

function showUsage(): void {
  const usage = `Rutgers SOC probe
Usage: npm run soc:probe -- --term <semester> --campus <code> [--subject <code>] [--endpoint courses|openSections]

Examples:
  npm run soc:probe -- --term 12024 --campus NB --subject 198
  npm run soc:probe -- --term FA2024 --campus NK --endpoint openSections

Flags:
  --term        Semester code (12024, 92024, FA2024, etc.)
  --campus      Campus code or comma list (NB, NK, CM, ONLINE_NB, ...)
  --subject     Optional subject code used for local filtering and request context
  --endpoint    courses (default) or openSections
  --samples     How many sample records to print (default: 3)
  --timeout     Request timeout in milliseconds (default: 15000)
  --level       Optional level hint (U,G,U,G) forwarded to the API
`;
  console.log(usage);
}

function parseArgs(): CLIOptions {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    showUsage();
    throw new CLIError('Missing required arguments.');
  }

  const opts: Partial<CLIOptions> = {
    endpoint: 'courses',
    sampleSize: 3,
    timeoutMs: 15000
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
      case 'endpoint': {
        const normalized = value as Endpoint;
        if (!ENDPOINTS.includes(normalized)) {
          throw new CLIError(`Unsupported endpoint: ${value}`);
        }
        opts.endpoint = normalized;
        break;
      }
      case 'samples': {
        const parsed = Number.parseInt(value, 10);
        if (Number.isNaN(parsed) || parsed <= 0) {
          throw new CLIError('samples must be a positive integer');
        }
        opts.sampleSize = parsed;
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
      case 'level':
        opts.level = value.toUpperCase();
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

  return opts as CLIOptions;
}


function summarizeCourses(body: unknown, subject: string | undefined, sampleSize: number): CourseSummary {
  if (!Array.isArray(body)) {
    return {
      totalRecords: 0,
      filteredRecords: 0,
      sample: [],
      note: 'Unexpected response shape for courses endpoint.'
    };
  }

  const normalizedSubject = subject?.toUpperCase();
  const filtered = normalizedSubject
    ? body.filter((course) => typeof course === 'object' && course !== null && (course as CourseRecord).subject?.toUpperCase() === normalizedSubject)
    : body;

  const sampleSource = filtered.length > 0 ? filtered : body;
  const sample = sampleSource.slice(0, sampleSize).map((course) => formatCourseSample(course as CourseRecord));
  const note = filtered.length === 0 && normalizedSubject
    ? `Subject ${normalizedSubject} not present in payload. Showing unfiltered sample instead.`
    : undefined;

  return {
    totalRecords: body.length,
    filteredRecords: normalizedSubject ? filtered.length : body.length,
    sample,
    note
  };
}

function formatCourseSample(course: CourseRecord): Record<string, unknown> {
  const campuses = course.campusLocations?.map((loc) => loc.description).filter(Boolean).join(', ');
  return {
    courseId: `${course.subject ?? 'UNK'}-${course.courseNumber ?? '????'}`,
    title: course.title ?? 'N/A',
    campuses: campuses || 'N/A',
    sections: Array.isArray(course.sections) ? course.sections.length : 0,
    openSections: course.openSections ?? 'N/A',
    credits: course.credits ?? 'N/A'
  };
}

function summarizeOpenSections(body: unknown, sampleSize: number): OpenSectionsSummary {
  if (!Array.isArray(body)) {
    return {
      totalRecords: 0,
      sample: []
    };
  }
  return {
    totalRecords: body.length,
    sample: body.slice(0, sampleSize).map((item) => String(item))
  };
}

function printSuccess(result: ProbeResult, options: CLIOptions, summary: CourseSummary | OpenSectionsSummary): void {
  console.log(`\n✅ ${options.endpoint} probe succeeded [${result.requestId}]`);
  console.log(`URL: ${result.url}`);
  console.log(`Status: ${result.statusCode} ${result.statusText} • ${result.durationMs.toFixed(1)} ms • ${result.sizeBytes.toLocaleString()} bytes decoded`);
  if ('filteredRecords' in summary) {
    console.log(`Records: total=${summary.totalRecords} filtered=${summary.filteredRecords} (subject=${options.subject ?? 'N/A'})`);
    if (summary.note) {
      console.log(`Note: ${summary.note}`);
    }
    if (summary.sample.length === 0) {
      console.log('No sample courses available.');
    } else {
      console.log('\nSample courses:');
      summary.sample.forEach((item, idx) => {
        console.log(`  [${idx + 1}] ${item.courseId} | ${item.title}`);
        console.log(`      campuses: ${item.campuses} • sections: ${item.sections} • open: ${item.openSections} • credits: ${item.credits}`);
      });
    }
  } else {
    console.log(`Open sections count: ${summary.totalRecords}`);
    if (summary.sample.length === 0) {
      console.log('No open section indexes returned.');
    } else {
      console.log(`Sample indexes: ${summary.sample.join(', ')}`);
    }
  }
  console.log('');
}

async function main(): Promise<void> {
  try {
    const options = parseArgs();
    let semester: SemesterParts;
    try {
      semester = decodeSemester(options.term);
    } catch (error) {
      throw new CLIError((error as Error).message);
    }
    const result = await performProbe(options, semester);
    if (options.endpoint === 'courses') {
      const summary = summarizeCourses(result.body, options.subject, options.sampleSize);
      printSuccess(result, options, summary);
    } else {
      const summary = summarizeOpenSections(result.body, options.sampleSize);
      printSuccess(result, options, summary);
    }
  } catch (error) {
    if (error instanceof CLIError) {
      console.error(`Argument error: ${error.message}`);
    } else if (error instanceof SOCRequestError) {
      const hint = error.retryHint ? ` (${error.retryHint})` : '';
      console.error(`Probe failed [${error.requestId}]: ${error.message}${hint}`);
    } else if (error instanceof Error) {
      console.error(`Probe failed: ${error.message}`);
    } else {
      console.error('Unknown error occurred.');
    }
    process.exit(1);
  }
}

main();
