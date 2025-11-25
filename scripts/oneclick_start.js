#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { evaluateMailConfig as evaluateMailConfigFile, summarizeIssues as summarizeTemplateIssues } from './mail_templates.js';

const MIN_NODE_MAJOR = 22;
const API_PORT = process.env.CSP_API_PORT ?? '3333';
const FRONTEND_PORT = process.env.CSP_FRONTEND_PORT ?? '5174';
const POLLER_INTERVAL = process.env.CSP_POLLER_INTERVAL ?? '15';
const POLLER_TERMS = process.env.CSP_TERMS ?? process.env.CSP_TERM ?? 'auto';
const POLLER_CAMPUSES = process.env.CSP_CAMPUSES;
const SKIP_POLLER = process.env.CSP_SKIP_POLLER === '1';
const FORCE_FETCH = process.env.CSP_FORCE_FETCH === '1';
const MAIL_BATCH = Number(process.env.CSP_MAIL_BATCH ?? '25');
const APP_BASE_URL = process.env.CSP_APP_BASE_URL ?? `http://localhost:${FRONTEND_PORT}`;

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT_DIR = path.resolve(process.cwd());
const FRONTEND_DIR = path.join(ROOT_DIR, 'frontend');
const FETCH_CONFIG_PATH = path.join(ROOT_DIR, 'configs', 'fetch_pipeline.local.json');
const FETCH_CONFIG_TEMPLATE = path.join(ROOT_DIR, 'configs', 'fetch_pipeline.example.json');
const CHECKPOINT_FILE = path.join(ROOT_DIR, 'data', 'poller_checkpoint.json');
const MAIL_CONFIG_DIR = resolveConfigDir(process.env.MAIL_CONFIG_DIR);
const MAIL_USER_CONFIG = path.join(MAIL_CONFIG_DIR, 'mail_sender.user.json');
const MAIL_LOCAL_CONFIG = path.join(MAIL_CONFIG_DIR, 'mail_sender.local.json');

const children = [];
let shuttingDown = false;

function sanitizeDbPath(rawDbPath) {
  let preferredDb = rawDbPath ?? path.join('data', 'fresh_local.db');

  if (process.platform === 'win32') {
    if (preferredDb.startsWith('/')) {
      warn(`Detected Unix-style sqliteFile (${preferredDb}); switching to data\\fresh_local.db.`);
      preferredDb = path.join('data', 'fresh_local.db');
    }
    if (preferredDb.toLowerCase().includes(`${path.sep}mnt${path.sep}`)) {
      warn(`Detected WSL-style sqliteFile (${preferredDb}); switching to data\\fresh_local.db.`);
      preferredDb = path.join('data', 'fresh_local.db');
    }
  }

  const resolved = path.isAbsolute(preferredDb) ? path.resolve(preferredDb) : path.resolve(ROOT_DIR, preferredDb);
  const resolvedDir = path.dirname(resolved);
  if (!fs.existsSync(resolvedDir)) {
    fs.mkdirSync(resolvedDir, { recursive: true });
  }
  return resolved;
}

function resolveConfigDir(rawDir) {
  if (!rawDir) {
    return path.join(ROOT_DIR, 'configs');
  }
  return path.isAbsolute(rawDir) ? rawDir : path.resolve(ROOT_DIR, rawDir);
}

function resolveNpmCli(baseCli) {
  const nodeDir = path.dirname(process.execPath);
  const cliPath = path.join(nodeDir, 'node_modules', 'npm', 'bin', baseCli);
  return fs.existsSync(cliPath) ? cliPath : null;
}

function resolveNpmLike(base) {
  if (process.platform !== 'win32') return base;
  const candidate = path.join(path.dirname(process.execPath), `${base}.cmd`);
  return fs.existsSync(candidate) ? candidate : `${base}.cmd`;
}

const NPM_CLI = resolveNpmCli('npm-cli.js');
const NPX_CLI = resolveNpmCli('npx-cli.js');
const NPM_CMD = process.platform === 'win32' && NPM_CLI ? process.execPath : resolveNpmLike('npm');
const NPX_CMD = process.platform === 'win32' && NPX_CLI ? process.execPath : resolveNpmLike('npx');

function withNpmArgs(args) {
  return process.platform === 'win32' && NPM_CLI ? [NPM_CLI, ...args] : args;
}

function withNpxArgs(args) {
  return process.platform === 'win32' && NPX_CLI ? [NPX_CLI, ...args] : args;
}

function log(message) {
  console.log(`[oneclick] ${message}`);
}

function warn(message) {
  console.warn(`[oneclick] ${message}`);
}

function quoteWindowsArg(arg) {
  if (!/[ \t"&]/.test(arg)) return arg;
  return `"${arg.replace(/(["\\])/g, '\\$1')}"`;
}

function prepareCommand(command, args) {
  if (process.platform !== 'win32' || !command.toLowerCase().endsWith('.cmd')) {
    return { command, args };
  }
  const cmdExe = process.env.ComSpec || 'cmd.exe';
  const full = [quoteWindowsArg(command), ...args.map(quoteWindowsArg)].join(' ').trim();
  return { command: cmdExe, args: ['/d', '/s', '/c', full] };
}

function ensureNodeVersion() {
  const major = Number(process.versions.node.split('.')[0]);
  if (Number.isNaN(major) || major < MIN_NODE_MAJOR) {
    console.error(
      `[oneclick] Node ${MIN_NODE_MAJOR}+ is required. Detected ${process.versions.node}. Install a newer Node.js from https://nodejs.org/.`,
    );
    process.exit(1);
  }
}

function parseCampuses(raw) {
  return raw
    .split(',')
    .map((code) => code.trim().toUpperCase())
    .filter(Boolean);
}

function readJsonSafe(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (error) {
    warn(`Could not read ${file}: ${error instanceof Error ? error.message : String(error)}`);
    return null;
  }
}

function inspectMailConfig() {
  const userStatus = evaluateMailConfigFile(MAIL_USER_CONFIG);
  if (userStatus.exists) {
    if (userStatus.parseError) {
      return {
        start: false,
        message: `Mail config at ${MAIL_USER_CONFIG} is invalid. 去 WebUI 填写邮件设置并关闭 dryRun 后重启。`,
      };
    }
    if (userStatus.templateIssues?.length) {
      const summary = summarizeTemplateIssues(userStatus.templateIssues);
      return {
        start: false,
        message: `Mail templates missing (${summary}). 补齐 templates/email 后再关闭 dryRun。`,
      };
    }
    if (userStatus.dryRun) {
      return {
        start: false,
        message: 'Mail dispatcher not started (dryRun=true in configs/mail_sender.user.json). 去 WebUI 填写邮件设置并关闭 dryRun 后重启。',
      };
    }
    if (!userStatus.hasKey) {
      const envHint = userStatus.apiKeyEnv ? ` (env ${userStatus.apiKeyEnv} not set)` : '';
      return {
        start: false,
        message: `Mail dispatcher not started: missing SendGrid API key${envHint}. 去 WebUI 填写邮件设置并关闭 dryRun 后重启。`,
      };
    }
    return { start: true, path: MAIL_USER_CONFIG, source: 'user', apiKeyEnv: userStatus.apiKeyEnv };
  }

  const fallbackStatus = evaluateMailConfigFile(MAIL_LOCAL_CONFIG);
  if (fallbackStatus.exists && fallbackStatus.templateIssues?.length) {
    const summary = summarizeTemplateIssues(fallbackStatus.templateIssues);
    return { start: false, message: `Mail templates missing (${summary}). 补齐 templates/email 后再关闭 dryRun。` };
  }

  if (fallbackStatus.exists && !fallbackStatus.parseError && !fallbackStatus.templateIssues?.length && fallbackStatus.hasKey) {
    return { start: true, path: MAIL_LOCAL_CONFIG, source: 'env', apiKeyEnv: fallbackStatus.apiKeyEnv };
  }
  if (fallbackStatus.parseError && fallbackStatus.exists) {
    return {
      start: false,
      message: `Mail config at ${MAIL_LOCAL_CONFIG} is invalid. 去 WebUI 填写邮件设置并关闭 dryRun 后重启。`,
    };
  }

  return { start: false, message: 'Mail dispatcher not started. 去 WebUI 填写邮件设置并关闭 dryRun 后重启。' };
}

function ensureFetchConfig() {
  if (!fs.existsSync(FETCH_CONFIG_PATH)) {
    if (!fs.existsSync(FETCH_CONFIG_TEMPLATE)) {
      console.error('[oneclick] Missing fetch config template under configs/.');
      process.exit(1);
    }
    fs.copyFileSync(FETCH_CONFIG_TEMPLATE, FETCH_CONFIG_PATH);
    log('Created configs/fetch_pipeline.local.json from example.');
  }

  const rawConfig = readJsonSafe(FETCH_CONFIG_PATH) ?? {};

  const envDb = process.env.CSP_DB_PATH ?? process.env.CSP_SQLITE_FILE ?? process.env.SQLITE_FILE;
  const defaultDb =
    typeof rawConfig.sqliteFile === 'string' && rawConfig.sqliteFile.length > 0
      ? rawConfig.sqliteFile
      : path.join('data', 'fresh_local.db');
  const dbPath = sanitizeDbPath(envDb ?? defaultDb);

  const targets = Array.isArray(rawConfig.targets) ? rawConfig.targets : [];
  const primaryTarget = targets[0] ?? {};
  const envTerm = process.env.CSP_TERM;
  const envCampuses = process.env.CSP_CAMPUSES;
  const term = envTerm ?? primaryTarget.term ?? '12024';
  const campuses = envCampuses ?? (Array.isArray(primaryTarget.campuses) ? primaryTarget.campuses.map((c) => c.code).join(',') : 'NB');
  const campusList = parseCampuses(campuses);

  let updated = false;
  if (rawConfig.sqliteFile !== dbPath) {
    rawConfig.sqliteFile = dbPath;
    updated = true;
  }
  if (!primaryTarget.term) {
    primaryTarget.term = term;
    updated = true;
  }
  if (!primaryTarget.mode) {
    primaryTarget.mode = 'full-init';
    updated = true;
  }
  if (!Array.isArray(primaryTarget.campuses) || primaryTarget.campuses.length === 0 || envCampuses) {
    primaryTarget.campuses = campusList.map((code) => ({ code, subjects: ['ALL'] }));
    updated = true;
  }
  if (!targets[0]) {
    rawConfig.targets = [primaryTarget];
  } else {
    rawConfig.targets[0] = primaryTarget;
  }

  if (updated) {
    fs.writeFileSync(FETCH_CONFIG_PATH, JSON.stringify(rawConfig, null, 2));
    log(`Updated fetch config (${FETCH_CONFIG_PATH}) to use ${dbPath}`);
  }

  return { dbPath, term, campuses: campusList, fetchConfigPath: FETCH_CONFIG_PATH };
}

function runCommand(label, command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const prepared = prepareCommand(command, args);
    log(`${label}...`);
    log(`  cmd: ${prepared.command} ${prepared.args.join(' ')} (cwd=${options.cwd ?? process.cwd()})`);
    // Avoid shell on Windows so paths with spaces (e.g., "C:\\Program Files\\") don't break.
    const child = spawn(prepared.command, prepared.args, { stdio: 'inherit', shell: false, ...options });
    child.on('exit', (code) => {
      if (code === 0) {
        resolve(0);
      } else {
        reject(new Error(`${label} failed (exit code ${code ?? 'unknown'})`));
      }
    });
    child.on('error', (error) =>
      reject(
        new Error(
          `${label} failed to start (${error instanceof Error ? `${error.message}` : String(error)}); cmd=${prepared.command} cwd=${options.cwd ?? process.cwd()}`,
        ),
      ),
    );
  });
}

async function ensureDependencies() {
  log(`Node runtime: ${process.platform} ${process.arch} ${process.versions.node}`);
  const tsxBin = path.join(
    ROOT_DIR,
    'node_modules',
    '.bin',
    process.platform === 'win32' ? 'tsx.cmd' : 'tsx',
  );
  const viteBin = path.join(
    FRONTEND_DIR,
    'node_modules',
    '.bin',
    process.platform === 'win32' ? 'vite.cmd' : 'vite',
  );

  const needsRootInstall =
    !fs.existsSync(path.join(ROOT_DIR, 'node_modules')) || (process.platform === 'win32' && !fs.existsSync(tsxBin));
  if (needsRootInstall) {
    await runCommand('Installing root dependencies (platform-specific rebuild)', NPM_CMD, withNpmArgs(['install', '--force']), {
      cwd: ROOT_DIR,
    });
  } else {
    log('Root dependencies already installed.');
  }

  const needsFrontendInstall =
    !fs.existsSync(path.join(FRONTEND_DIR, 'node_modules')) ||
    (process.platform === 'win32' && !fs.existsSync(viteBin));
  if (needsFrontendInstall) {
    await runCommand('Installing frontend dependencies (platform-specific rebuild)', NPM_CMD, withNpmArgs(['install', '--force']), {
      cwd: FRONTEND_DIR,
    });
  } else {
    log('Frontend dependencies already installed.');
  }

  // Validate better-sqlite3 binary matches the platform; if not, force reinstall.
  const validate = spawn(process.execPath, ['-e', "require('better-sqlite3')"], {
    cwd: ROOT_DIR,
    stdio: 'inherit',
    shell: false,
  });
  const exitCode = await new Promise((resolve) => {
    validate.on('error', () => resolve(1));
    validate.on('exit', (code) => resolve(typeof code === 'number' ? code : 1));
  });
  if (exitCode !== 0) {
    warn('Detected broken better-sqlite3 binary; retrying install for this platform...');
    const betterSqliteDir = path.join(ROOT_DIR, 'node_modules', 'better-sqlite3');
    try {
      if (fs.existsSync(betterSqliteDir)) {
        fs.rmSync(betterSqliteDir, { recursive: true, force: true });
      }
    } catch (err) {
      warn(`Could not clean old better-sqlite3 folder: ${err instanceof Error ? err.message : String(err)}`);
    }
    await runCommand('Reinstalling better-sqlite3', NPM_CMD, withNpmArgs(['install', '--force', 'better-sqlite3']), {
      cwd: ROOT_DIR,
      env: { ...process.env, npm_config_build_from_source: '1' },
    });
    const revalidate = spawn(process.execPath, ['-e', "require('better-sqlite3')"], {
      cwd: ROOT_DIR,
      stdio: 'inherit',
      shell: false,
    });
    const revalidateExit = await new Promise((resolve) => {
      revalidate.on('error', () => resolve(1));
      revalidate.on('exit', (code) => resolve(typeof code === 'number' ? code : 1));
    });
    if (revalidateExit !== 0) {
      warn('better-sqlite3 still failing; doing a clean reinstall of all dependencies (this may take a bit)...');
      try {
        fs.rmSync(path.join(ROOT_DIR, 'node_modules'), { recursive: true, force: true });
      } catch (err) {
        warn(`Could not remove node_modules completely: ${err instanceof Error ? err.message : String(err)}`);
      }
      await runCommand('Reinstalling all dependencies cleanly', NPM_CMD, withNpmArgs(['install', '--force']), {
        cwd: ROOT_DIR,
        env: { ...process.env, npm_config_build_from_source: '1' },
      });
      const finalValidate = spawn(process.execPath, ['-e', "require('better-sqlite3')"], {
        cwd: ROOT_DIR,
        stdio: 'inherit',
        shell: false,
      });
      const finalExit = await new Promise((resolve) => {
        finalValidate.on('error', () => resolve(1));
        finalValidate.on('exit', (code) => resolve(typeof code === 'number' ? code : 1));
      });
      if (finalExit !== 0) {
        throw new Error(
          'better-sqlite3 is still not loading. Please install "Microsoft C++ Build Tools" (Desktop development with C++) and then rerun Start-WebUI.bat, or manually delete node_modules and rerun.',
        );
      }
    }
  }
}

async function prepareDatabase(dbPath, term, campuses, fetchConfigPath) {
  fs.mkdirSync(path.dirname(dbPath), { recursive: true });
  await runCommand(
    'Running database migrations',
    NPM_CMD,
    withNpmArgs(['run', 'db:migrate', '--', '--db', dbPath, '--verbose']),
    {
      cwd: ROOT_DIR,
    },
  );

  const needsFetch = FORCE_FETCH || !fs.existsSync(dbPath);
  if (!needsFetch) {
    log('Database already exists; skipping full fetch. Set CSP_FORCE_FETCH=1 to refresh.');
    return;
  }

  await runCommand(
    'Fetching course data (this can take a few minutes on first run)',
    NPM_CMD,
    withNpmArgs(['run', 'data:fetch', '--', '--config', fetchConfigPath, '--mode', 'full-init', '--terms', term, '--campuses', campuses.join(',')]),
    { cwd: ROOT_DIR },
  );
}

function startProcess(name, command, args, options = {}) {
  log(`Starting ${name}...`);
  const prepared = prepareCommand(command, args);
  const child = spawn(prepared.command, prepared.args, { stdio: 'inherit', shell: false, ...options });
  children.push({ name, child });
  child.on('exit', (code, signal) => {
    if (shuttingDown) return;
    const reason = signal ? `signal ${signal}` : `exit code ${code ?? 'unknown'}`;
    console.error(`[oneclick] ${name} stopped (${reason}). Shutting down.`);
    cleanup(typeof code === 'number' ? code : 1);
  });
  child.on('error', (error) => {
    console.error(`[oneclick] Failed to start ${name}: ${error instanceof Error ? error.message : String(error)}`);
    cleanup(1);
  });
}

function cleanup(exitCode = 0) {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;
  for (const { child } of children) {
    if (!child.killed) {
      child.kill();
    }
  }
  process.exit(exitCode);
}

function openBrowser(url) {
  let command;
  let args;

  if (process.platform === 'win32') {
    command = 'cmd';
    args = ['/c', 'start', '', url];
  } else if (process.platform === 'darwin') {
    command = 'open';
    args = [url];
  } else {
    command = 'xdg-open';
    args = [url];
  }

  const opener = spawn(command, args, { stdio: 'ignore', detached: true });
  opener.unref();
}

async function main() {
  console.log('=== BetterCourseSchedulePlanner one-click launcher ===');
  log('If you close this window, the servers will stop.');
  ensureNodeVersion();

  const { dbPath, term, campuses, fetchConfigPath } = ensureFetchConfig();
  const pollerTermsRaw = POLLER_TERMS;
  const pollerAuto = pollerTermsRaw.toLowerCase() === 'auto';
  const pollerAllowlist = POLLER_CAMPUSES ? parseCampuses(POLLER_CAMPUSES) : [];
  const pollerCampuses = pollerAuto ? pollerAllowlist : pollerAllowlist.length ? pollerAllowlist : campuses;

  log(`Using SQLite DB at: ${dbPath}`);
  log(`Fetch target (configs/fetch_pipeline.local.json): term=${term} | campuses=${campuses.join(',')}`);
  if (pollerAuto) {
    const allowlistText = pollerCampuses.length ? pollerCampuses.join(',') : 'none';
    log(
      `Poller terms=auto (discover subscriptions); campus allowlist: ${allowlistText}. Missing term/campus data will log "fetch course data"—fetch that combo before expecting notifications.`,
    );
  } else {
    log(`Poller terms=${pollerTermsRaw} campuses=${pollerCampuses.join(',')}`);
  }
  if (!pollerAuto && pollerCampuses.length === 0) {
    throw new Error('Explicit poller mode requires campuses. Set CSP_CAMPUSES or update fetch_pipeline.local.json targets[].');
  }

  await ensureDependencies();
  await prepareDatabase(dbPath, term, campuses, fetchConfigPath);

  startProcess(
    'api',
    NPM_CMD,
    withNpmArgs(['run', 'api:start']),
    {
      cwd: ROOT_DIR,
      env: { ...process.env, APP_PORT: API_PORT, APP_HOST: '127.0.0.1', SQLITE_FILE: dbPath },
    },
  );

  startProcess(
    'frontend',
    NPM_CMD,
    withNpmArgs(['run', 'dev', '--', '--host', '127.0.0.1', '--port', FRONTEND_PORT]),
    {
      cwd: FRONTEND_DIR,
      env: {
        ...process.env,
        VITE_API_PROXY_TARGET: `http://localhost:${API_PORT}`,
        VITE_API_BASE_URL: '/api',
      },
    },
  );

  if (!SKIP_POLLER) {
    fs.mkdirSync(path.dirname(CHECKPOINT_FILE), { recursive: true });
    const pollerArgs = [
      'tsx',
      'workers/open_sections_poller.ts',
      '--terms',
      pollerTermsRaw,
      '--sqlite',
      dbPath,
      '--interval',
      POLLER_INTERVAL,
      '--checkpoint',
      CHECKPOINT_FILE,
    ];
    if (pollerCampuses.length) {
      pollerArgs.push('--campuses', pollerCampuses.join(','));
    }
    startProcess(
      'open_sections_poller',
      NPX_CMD,
      withNpxArgs(pollerArgs),
      { cwd: ROOT_DIR },
    );
  } else {
    log('Skipping poller (CSP_SKIP_POLLER=1).');
  }

  const mailDecision = inspectMailConfig();
  if (mailDecision.start && mailDecision.path) {
    startProcess(
      'mail_dispatcher',
      NPX_CMD,
      withNpxArgs([
        'tsx',
        'workers/mail_dispatcher.ts',
        '--sqlite',
        dbPath,
        '--mail-config',
        mailDecision.path,
        '--batch',
        String(MAIL_BATCH),
        '--app-base-url',
        APP_BASE_URL,
      ]),
      { cwd: ROOT_DIR },
    );
    const envNote = mailDecision.apiKeyEnv ? ` (${mailDecision.apiKeyEnv})` : '';
    log(
      mailDecision.source === 'user'
        ? 'Detected configs/mail_sender.user.json with dryRun=false; mail dispatcher started automatically.'
        : `Mail dispatcher started using configs/mail_sender.local.json${envNote ? ` and ${envNote}` : ''}.`,
    );
  } else if (mailDecision.message) {
    log(mailDecision.message);
  }

  const uiUrl = `http://localhost:${FRONTEND_PORT}`;
  setTimeout(() => openBrowser(uiUrl), 1500);
  log(`Opening the web UI at ${uiUrl} ...`);
  log('Press Ctrl+C in this window to stop the stack.');
}

process.on('SIGINT', () => {
  log('Shutting down...');
  cleanup(0);
});

process.on('SIGTERM', () => {
  log('Shutting down...');
  cleanup(0);
});

main().catch((error) => {
  console.error(`[oneclick] ${error instanceof Error ? error.message : String(error)}`);
  cleanup(1);
});
