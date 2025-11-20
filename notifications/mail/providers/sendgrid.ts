import { renderMessageContent, TemplateError } from '../template_loader.js';
import type {
  MailMessage,
  MailSender,
  RenderedContent,
  ResolvedMailSenderConfig,
  SendErrorCode,
  SendOptions,
  SendResult,
} from '../types.js';

const PROVIDER: SendResult['provider'] = 'sendgrid';
const DEFAULT_API_BASE = 'https://api.sendgrid.com';

type SendGridPayload = {
  personalizations: Array<{
    to: Array<{ email: string; name?: string }>;
    subject?: string;
    headers?: Record<string, string>;
    custom_args?: Record<string, string>;
  }>;
  from: { email: string; name?: string };
  reply_to?: { email: string; name?: string };
  content: Array<{ type: 'text/plain' | 'text/html'; value: string }>;
  attachments?: Array<{ filename: string; type: string; content: string; disposition: 'attachment' }>;
  mail_settings?: { sandbox_mode?: { enable: boolean } };
  categories?: string[];
  ip_pool_name?: string | null;
};

export class SendGridMailSender implements MailSender {
  constructor(private readonly config: ResolvedMailSenderConfig) {}

  async send(message: MailMessage, options: SendOptions = {}): Promise<SendResult> {
    const attempt = 1;
    const startedAt = Date.now();

    const cfg = this.config.providers.sendgrid;
    if (!cfg) {
      return this.failure(attempt, startedAt, 'validation_error', 'SendGrid provider is not configured');
    }
    if (options.providerOverride && options.providerOverride !== PROVIDER) {
      return this.failure(
        attempt,
        startedAt,
        'validation_error',
        `providerOverride=${options.providerOverride} is not supported by SendGrid adapter`,
      );
    }

    const normalizedRecipient = this.applyRecipientOverride(message.to.email);
    if (!isValidEmail(normalizedRecipient)) {
      return this.failure(attempt, startedAt, 'invalid_recipient', 'Recipient email is invalid');
    }
    const normalizedFrom = message.from ?? this.config.defaultFrom;
    if (!isValidEmail(normalizedFrom.email)) {
      return this.failure(attempt, startedAt, 'validation_error', 'From email is invalid');
    }
    const replyTo = message.replyTo ?? this.config.replyTo;
    if (replyTo && !isValidEmail(replyTo.email)) {
      return this.failure(attempt, startedAt, 'validation_error', 'Reply-To email is invalid');
    }

    let rendered: RenderedContent;
    try {
      rendered = await renderMessageContent(this.config, message);
    } catch (error) {
      if (error instanceof TemplateError) {
        return this.failure(attempt, startedAt, error.code, error.message);
      }
      return this.failure(attempt, startedAt, 'unknown', error instanceof Error ? error.message : 'Unknown template error');
    }

    if (options.dryRun || this.config.testHooks?.dryRun) {
      return this.success(attempt, startedAt, { providerMessageId: 'dry-run' });
    }

    const payload = buildPayload({
      cfg,
      message,
      rendered,
      to: { email: normalizedRecipient, name: message.to.name },
      from: normalizedFrom,
      replyTo,
      traceHeader: this.config.logging?.traceHeader ?? 'X-Trace-Id',
    });

    const controller = new AbortController();
    const timeoutMs = options.timeoutMs ?? this.config.timeouts.sendMs;
    const timeout = setAbort(controller, timeoutMs);
    try {
      const response = await fetch(`${cfg.apiBaseUrl ?? DEFAULT_API_BASE}/v3/mail/send`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${cfg.apiKey}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(payload),
        signal: controller.signal,
      });

      return await this.handleResponse(response, attempt, startedAt);
    } catch (error) {
      if (isAbortError(error)) {
        return this.retryable(attempt, startedAt, 'network_error', `SendGrid request timed out after ${timeoutMs}ms`);
      }
      return this.retryable(attempt, startedAt, 'network_error', error instanceof Error ? error.message : 'Unknown network error');
    } finally {
      clearTimeout(timeout);
    }
  }

  private async handleResponse(response: Response, attempt: number, startedAt: number): Promise<SendResult> {
    if (response.status === 202) {
      const providerMessageId = response.headers.get('x-message-id') ?? undefined;
      return this.success(attempt, startedAt, { providerMessageId });
    }

    const body = await safeReadBody(response);
    const retryAfterSeconds = parseRetryAfter(response.headers.get('retry-after'));

    if (response.status === 429) {
      return this.retryable(attempt, startedAt, 'rate_limited', body ?? 'rate limited', retryAfterSeconds ?? 30);
    }
    if (response.status >= 500) {
      return this.retryable(attempt, startedAt, 'provider_error', body ?? 'provider 5xx error', retryAfterSeconds);
    }
    if (response.status === 401 || response.status === 403) {
      return this.failure(attempt, startedAt, 'unauthorized', body ?? 'unauthorized');
    }
    if (response.status >= 400) {
      return this.failure(attempt, startedAt, 'provider_error', body ?? `provider returned ${response.status}`);
    }

    return this.failure(attempt, startedAt, 'unknown', body ?? `unexpected status ${response.status}`);
  }

  private applyRecipientOverride(original: string) {
    const override = this.config.testHooks?.overrideRecipient;
    return normalizeEmail(override ?? original);
  }

  private success(attempt: number, startedAt: number, extra?: { providerMessageId?: string }): SendResult {
    return {
      status: 'sent',
      provider: PROVIDER,
      attempt,
      durationMs: Date.now() - startedAt,
      sentAt: new Date().toISOString(),
      providerMessageId: extra?.providerMessageId,
    };
  }

  private failure(attempt: number, startedAt: number, code: SendErrorCode, message: string): SendResult {
    return {
      status: 'failed',
      provider: PROVIDER,
      attempt,
      durationMs: Date.now() - startedAt,
      error: { code, message },
    };
  }

  private retryable(
    attempt: number,
    startedAt: number,
    code: SendErrorCode,
    message: string,
    retryAfterSeconds?: number,
  ): SendResult {
    return {
      status: 'retryable',
      provider: PROVIDER,
      attempt,
      durationMs: Date.now() - startedAt,
      retryAfterSeconds,
      error: { code, message },
    };
  }
}

function buildPayload(input: {
  cfg: NonNullable<ResolvedMailSenderConfig['providers']['sendgrid']>;
  message: MailMessage;
  rendered: RenderedContent;
  to: { email: string; name?: string };
  from: { email: string; name?: string };
  replyTo?: { email: string; name?: string };
  traceHeader: string;
}): SendGridPayload {
  const attachments = input.message.attachments?.map((att) => ({
    filename: att.filename,
    type: att.mimeType,
    content: att.contentBase64,
    disposition: 'attachment' as const,
  }));

  const headers = input.message.traceId ? { [input.traceHeader]: input.message.traceId } : undefined;
  const customArgs: Record<string, string> = {};
  if (input.message.dedupeKey) customArgs.dedupe_key = input.message.dedupeKey;
  if (input.message.traceId) customArgs.trace_id = input.message.traceId;
  if (input.message.templateVersion) customArgs.template_version = input.message.templateVersion;
  if (input.message.metadata?.subscriptionId) customArgs.subscription_id = String(input.message.metadata.subscriptionId);
  if (input.message.metadata?.openEventId) customArgs.open_event_id = String(input.message.metadata.openEventId);

  const content: SendGridPayload['content'] = [];
  if (input.rendered.textBody) {
    content.push({ type: 'text/plain', value: input.rendered.textBody });
  }
  if (input.rendered.htmlBody) {
    content.push({ type: 'text/html', value: input.rendered.htmlBody });
  }

  return {
    personalizations: [
      {
        to: [{ email: input.to.email, ...(input.to.name ? { name: input.to.name } : {}) }],
        subject: input.rendered.subject || undefined,
        headers,
        custom_args: Object.keys(customArgs).length ? customArgs : undefined,
      },
    ],
    from: input.from,
    reply_to: input.replyTo,
    content,
    attachments,
    mail_settings: {
      sandbox_mode: { enable: input.cfg.sandboxMode ?? false },
    },
    categories: input.cfg.categories?.length ? input.cfg.categories : undefined,
    ip_pool_name: input.cfg.ipPool ?? undefined,
  };
}

function normalizeEmail(email: string) {
  return email.trim().toLowerCase();
}

function isValidEmail(email: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

function parseRetryAfter(value: string | null): number | undefined {
  if (!value) return undefined;
  const seconds = Number.parseInt(value, 10);
  return Number.isFinite(seconds) ? seconds : undefined;
}

async function safeReadBody(response: Response): Promise<string | undefined> {
  try {
    const text = await response.text();
    return text || undefined;
  } catch {
    return undefined;
  }
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === 'AbortError';
}

function setAbort(controller: AbortController, timeoutMs: number) {
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  // Avoid keeping the event loop alive solely for the timer.
  if (typeof timer === 'object' && 'unref' in timer && typeof timer.unref === 'function') {
    timer.unref();
  }
  return timer;
}
