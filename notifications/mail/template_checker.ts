import fs from 'node:fs/promises';
import path from 'node:path';

import type { MailSenderConfig, ResolvedMailSenderConfig } from './types.js';

export type TemplateIssue = {
  templateId: string;
  locale?: string;
  kind?: 'html' | 'text';
  path: string;
  message: string;
};

export async function collectTemplateIssues(
  config: MailSenderConfig | ResolvedMailSenderConfig,
  baseDir: string = process.cwd(),
): Promise<TemplateIssue[]> {
  const issues: TemplateIssue[] = [];
  const templateRootValue = typeof config.templateRoot === 'string' ? config.templateRoot : null;

  if (!templateRootValue) {
    issues.push({
      templateId: 'root',
      path: '',
      message: 'templateRoot is missing or invalid',
    });
    return issues;
  }

  const templateRoot = path.isAbsolute(templateRootValue) ? templateRootValue : path.resolve(baseDir, templateRootValue);
  const rootExists = await pathExists(templateRoot);
  if (!rootExists) {
    issues.push({
      templateId: 'root',
      path: templateRoot,
      message: `Template root not found: ${templateRoot}`,
    });
    return issues;
  }

  const templates = config.templates ?? {};
  for (const [templateId, definition] of Object.entries(templates)) {
    issues.push(...(await collectRecordIssues(templateId, definition.html, templateRoot, 'html')));
    if (definition.text) {
      issues.push(...(await collectRecordIssues(templateId, definition.text, templateRoot, 'text')));
    }
  }

  return issues;
}

async function collectRecordIssues(
  templateId: string,
  record: Record<string, string>,
  templateRoot: string,
  kind: 'html' | 'text',
): Promise<TemplateIssue[]> {
  const issues: TemplateIssue[] = [];
  for (const [locale, relativePath] of Object.entries(record ?? {})) {
    const resolved = resolveTemplatePath(templateRoot, relativePath);
    if (!resolved) continue;
    const exists = await pathExists(resolved);
    if (!exists) {
      issues.push({
        templateId,
        locale,
        kind,
        path: resolved,
        message: `Missing ${kind} template for ${templateId} (${locale}): ${resolved}`,
      });
    }
  }
  return issues;
}

function resolveTemplatePath(templateRoot: string, relativePath: string | undefined): string | null {
  if (!relativePath) return null;
  return path.isAbsolute(relativePath) ? relativePath : path.resolve(templateRoot, relativePath);
}

async function pathExists(filePath: string): Promise<boolean> {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}
