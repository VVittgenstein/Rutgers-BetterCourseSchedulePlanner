export type ProviderType = 'sendgrid' | 'smtp';

export interface MailSender {
  send(message: MailMessage, options?: SendOptions): Promise<SendResult>;
  verify?(options?: VerifyOptions): Promise<VerifyResult>;
}

export interface MailMessage {
  to: { email: string; name?: string };
  from?: { email: string; name?: string };
  replyTo?: { email: string; name?: string };
  locale: string;
  templateId: string;
  templateVersion?: string;
  subject?: string;
  variables: Record<string, string | number | boolean | null>;
  textBody?: string;
  htmlBody?: string;
  unsubscribeUrl?: string;
  manageUrl?: string;
  attachments?: Array<{ filename: string; mimeType: string; contentBase64: string }>;
  dedupeKey?: string;
  traceId?: string;
  metadata?: { subscriptionId?: number; openEventId?: number; term?: string; campus?: string };
}

export interface SendOptions {
  timeoutMs?: number;
  providerOverride?: ProviderType;
  rateLimitKey?: string;
  dryRun?: boolean;
}

export type SendStatus = 'sent' | 'retryable' | 'failed';
export type SendErrorCode =
  | 'validation_error'
  | 'template_missing_locale'
  | 'template_variable_missing'
  | 'invalid_recipient'
  | 'rate_limited'
  | 'unauthorized'
  | 'network_error'
  | 'provider_error'
  | 'unknown';

export interface SendResult {
  status: SendStatus;
  provider: ProviderType;
  providerMessageId?: string;
  attempt: number;
  durationMs: number;
  sentAt?: string;
  retryAfterSeconds?: number;
  error?: { code: SendErrorCode; message: string; detail?: unknown };
}

export interface VerifyOptions {
  timeoutMs?: number;
}

export interface VerifyResult {
  status: 'ok' | 'failed';
  provider: ProviderType;
  durationMs: number;
  error?: { code: SendErrorCode; message: string };
}

export type TemplateDefinition = {
  subject?: Record<string, string>;
  html: Record<string, string>;
  text?: Record<string, string>;
  requiredVariables: string[];
};

export type RateLimitConfig = {
  maxPerSecond: number;
  burst: number;
  bucketWidthSeconds: number;
};

export type RetryPolicyConfig = {
  maxAttempts: number;
  backoffMs: number[];
  jitter: number;
  retryableErrors: SendErrorCode[];
};

export type MailSenderConfig = {
  provider: ProviderType;
  defaultFrom: { email: string; name?: string };
  replyTo?: { email: string; name?: string };
  supportedLocales: string[];
  templateRoot: string;
  templates: Record<string, TemplateDefinition>;
  rateLimit?: RateLimitConfig;
  retryPolicy?: RetryPolicyConfig;
  timeouts: {
    connectMs: number;
    sendMs: number;
    idleMs: number;
  };
  providers: {
    sendgrid?: SendGridConfig;
    smtp?: SMTPConfig;
  };
  logging?: {
    redactPII?: boolean;
    traceHeader?: string;
  };
  testHooks?: {
    dryRun?: boolean;
    overrideRecipient?: string | null;
  };
};

export type SendGridConfig = {
  apiKey?: string;
  apiKeyEnv?: string;
  sandboxMode?: boolean;
  categories?: string[];
  ipPool?: string | null;
  apiBaseUrl?: string;
};

export type ResolvedSendGridConfig = Omit<SendGridConfig, 'apiKeyEnv' | 'apiKey'> & {
  apiKey: string;
  apiKeyEnv?: string;
};

export type SMTPConfig = {
  host: string;
  port: number;
  secure: boolean;
  username?: string;
  passwordEnv?: string;
  poolSize?: number;
  tls?: { rejectUnauthorized?: boolean };
};

export type ResolvedSMTPConfig = Omit<SMTPConfig, 'passwordEnv'> & {
  password?: string;
};

export type ResolvedMailSenderConfig = Omit<MailSenderConfig, 'providers' | 'rateLimit' | 'retryPolicy'> & {
  rateLimit: RateLimitConfig;
  retryPolicy: RetryPolicyConfig;
  providers: {
    sendgrid?: ResolvedSendGridConfig;
    smtp?: ResolvedSMTPConfig;
  };
};

export type MailSendAttempt = {
  attempt: number;
  startedAt: string;
  finishedAt: string;
  durationMs: number;
  waitMs?: number;
  nextDelayMs?: number;
  result: SendResult;
};

export type SendWithRetryResult = {
  finalResult: SendResult;
  attempts: MailSendAttempt[];
};

export type RenderedContent = {
  subject: string;
  htmlBody?: string;
  textBody?: string;
};
