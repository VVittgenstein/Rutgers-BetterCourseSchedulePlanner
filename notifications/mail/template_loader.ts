import fs from 'node:fs/promises';
import path from 'node:path';

import type { MailMessage, RenderedContent, ResolvedMailSenderConfig, SendErrorCode } from './types.js';

const PLACEHOLDER_PATTERN = /{{\s*([a-zA-Z0-9_]+)\s*}}/g;

export class TemplateError extends Error {
  constructor(public readonly code: SendErrorCode, message: string) {
    super(message);
  }
}

export async function renderMessageContent(config: ResolvedMailSenderConfig, message: MailMessage): Promise<RenderedContent> {
  if (!config.supportedLocales.includes(message.locale)) {
    throw new TemplateError('validation_error', `Locale ${message.locale} is not supported`);
  }

  const template = config.templates[message.templateId];
  if (!template) {
    throw new TemplateError('template_missing_locale', `Template ${message.templateId} not configured`);
  }

  const variables = buildVariables(message);
  const missing = template.requiredVariables.filter((key) => variables[key] === undefined || variables[key] === null);
  if (missing.length) {
    throw new TemplateError('template_variable_missing', `Missing template variables: ${missing.join(', ')}`);
  }

  const subjectTemplate = message.subject ?? template.subject?.[message.locale];
  const subject = subjectTemplate ? renderTemplateString(subjectTemplate, variables, { escapeHtml: false }) : '';

  const htmlBody =
    message.htmlBody ??
    (await renderFileIfPresent(config.templateRoot, template.html[message.locale], variables, { escapeHtml: true }));

  const textTemplatePath = template.text?.[message.locale];
  const textBody =
    message.textBody ?? (textTemplatePath ? await renderFileIfPresent(config.templateRoot, textTemplatePath, variables) : undefined);

  if (!htmlBody && !textBody) {
    throw new TemplateError('template_missing_locale', `No html/text body found for template ${message.templateId} locale ${message.locale}`);
  }

  return {
    subject,
    htmlBody: htmlBody ?? undefined,
    textBody: textBody ?? undefined,
  };
}

export function renderTemplateString(
  template: string,
  variables: Record<string, string | number | boolean | null | undefined>,
  opts: { escapeHtml?: boolean } = {},
): string {
  const escapeHtml = opts.escapeHtml !== false ? opts.escapeHtml ?? false : false;
  return template.replace(PLACEHOLDER_PATTERN, (_, key: string) => {
    const raw = variables[key];
    if (raw === undefined || raw === null) return '';
    const value = formatVariable(raw);
    return escapeHtml ? escapeHtmlValue(value) : value;
  });
}

function buildVariables(message: MailMessage): Record<string, string | number | boolean | null | undefined> {
  const merged = { ...message.variables };
  if (merged.locale === undefined) {
    merged.locale = message.locale;
  }
  if (message.unsubscribeUrl && merged.unsubscribeUrl === undefined) {
    merged.unsubscribeUrl = message.unsubscribeUrl;
  }
  if (message.manageUrl && merged.manageUrl === undefined) {
    merged.manageUrl = message.manageUrl;
  }
  return merged;
}

async function renderFileIfPresent(
  templateRoot: string,
  relativePath: string | undefined,
  variables: Record<string, string | number | boolean | null | undefined>,
  opts: { escapeHtml?: boolean } = {},
): Promise<string | null> {
  if (!relativePath) return null;
  const resolved = path.isAbsolute(relativePath) ? relativePath : path.resolve(templateRoot, relativePath);
  try {
    const contents = await fs.readFile(resolved, 'utf8');
    return renderTemplateString(contents, variables, opts);
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Failed to load template';
    if ((error as NodeJS.ErrnoException)?.code === 'ENOENT') {
      throw new TemplateError('template_missing_locale', `Template file not found: ${resolved}`);
    }
    throw new TemplateError('unknown', message);
  }
}

function formatVariable(value: string | number | boolean): string {
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  return String(value);
}

function escapeHtmlValue(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}
