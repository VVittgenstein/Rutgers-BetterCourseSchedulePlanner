import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function collectTemplateIssues(config, baseDir = process.cwd()) {
  const issues = [];
  const templateRootValue = typeof config?.templateRoot === 'string' ? config.templateRoot : null;
  if (!templateRootValue) {
    issues.push({ templateId: 'root', path: '', message: 'templateRoot is missing or invalid' });
    return issues;
  }

  const templateRoot = path.isAbsolute(templateRootValue) ? templateRootValue : path.resolve(baseDir, templateRootValue);
  if (!exists(templateRoot)) {
    issues.push({ templateId: 'root', path: templateRoot, message: `Template root not found: ${templateRoot}` });
    return issues;
  }

  const templates = config?.templates && typeof config.templates === 'object' ? config.templates : {};
  for (const [templateId, definition] of Object.entries(templates)) {
    collectRecordIssues(templateId, definition?.html, templateRoot, 'html', issues);
    collectRecordIssues(templateId, definition?.text, templateRoot, 'text', issues);
  }
  return issues;
}

export function evaluateMailConfig(configPath) {
  const resolved = path.resolve(configPath);
  if (!fs.existsSync(resolved)) {
    return { exists: false, path: resolved };
  }
  let config;
  try {
    config = JSON.parse(fs.readFileSync(resolved, 'utf8'));
  } catch (error) {
    return { exists: true, path: resolved, parseError: error instanceof Error ? error.message : String(error) };
  }

  const dryRun = Boolean(config?.testHooks?.dryRun);
  const sendgrid = config?.providers?.sendgrid ?? {};
  const apiKey = typeof sendgrid.apiKey === 'string' ? sendgrid.apiKey.trim() : '';
  const apiKeyEnv = typeof sendgrid.apiKeyEnv === 'string' ? sendgrid.apiKeyEnv.trim() : '';
  const envValue = apiKeyEnv ? process.env[apiKeyEnv] : undefined;
  const hasKey = Boolean(apiKey || envValue);
  const templateIssues = collectTemplateIssues(config, path.dirname(resolved));

  return {
    exists: true,
    path: resolved,
    parseError: undefined,
    dryRun,
    hasKey,
    apiKeyEnv,
    templateIssues,
  };
}

export function summarizeIssues(issues, limit = 3) {
  if (!issues || issues.length === 0) return '';
  const head = issues.slice(0, limit).map((issue) => issue.path ?? issue.message ?? issue.templateId ?? 'unknown');
  const remaining = issues.length - head.length;
  return `${head.join(', ')}${remaining > 0 ? ` (+${remaining} more)` : ''}`;
}

function collectRecordIssues(templateId, record, templateRoot, kind, issues) {
  if (!record || typeof record !== 'object') return;
  for (const [locale, relativePath] of Object.entries(record)) {
    if (!relativePath) continue;
    const resolved = path.isAbsolute(relativePath) ? relativePath : path.resolve(templateRoot, relativePath);
    if (!exists(resolved)) {
      issues.push({
        templateId,
        locale,
        kind,
        path: resolved,
        message: `Missing ${kind} template for ${templateId} (${locale}): ${resolved}`,
      });
    }
  }
}

function exists(filePath) {
  try {
    fs.accessSync(filePath, fs.constants.R_OK);
    return true;
  } catch {
    return false;
  }
}

function printStatus(status, detail = '') {
  const sanitized = detail.replace(/\s+/g, ' ').trim();
  console.log(`${status}|${sanitized}`);
}

function isMain() {
  return fileURLToPath(import.meta.url) === path.resolve(process.argv[1] ?? '');
}

function handleCli() {
  const [, , command, configPath] = process.argv;
  if (command !== 'status' || !configPath) {
    printStatus('error', 'command requires: status <configPath>');
    process.exit(1);
  }

  const result = evaluateMailConfig(configPath);
  if (!result.exists) {
    printStatus('missing', '');
    return;
  }
  if (result.parseError) {
    printStatus('error', result.parseError);
    return;
  }
  if (result.templateIssues?.length) {
    printStatus('missing-templates', summarizeIssues(result.templateIssues));
    return;
  }
  if (result.dryRun) {
    printStatus('dryrun', '');
    return;
  }
  if (!result.hasKey) {
    printStatus('missing-key', result.apiKeyEnv ?? '');
    return;
  }
  printStatus('start', result.apiKeyEnv ?? '');
}

if (isMain()) {
  handleCli();
}
