import type { DiscordMessagePayload, DiscordSendRequest, DiscordTarget, ResolvedDiscordBotConfig } from './bot.js';

type DiscordContactType = 'discord_user' | 'discord_channel';

export type SectionRow = {
  section_id: number;
  course_id: number;
  term_id: string;
  campus_code: string;
  subject_code: string;
  section_number: string | null;
  index_number: string;
  open_status: string | null;
  open_status_updated_at: string | null;
  meeting_mode_summary: string | null;
};

export type CourseRow = {
  course_id: number;
  subject_code: string;
  course_number: string;
  course_string: string | null;
  title: string;
};

export type MeetingRow = {
  section_id: number;
  meeting_day: string | null;
  start_minutes: number | null;
  end_minutes: number | null;
  campus_abbrev: string | null;
  campus_location_code: string | null;
  campus_location_desc: string | null;
  building_code: string | null;
  room_number: string | null;
};

export type DiscordFanoutJob = {
  dedupeKey: string;
  event: {
    openEventId: number;
    termId: string;
    campusCode: string;
    indexNumber: string;
    statusAfter: string | null;
    seatDelta: number | null;
    eventAt: string;
    traceId: string | null;
    payload: Record<string, unknown>;
  };
  subscription: {
    subscriptionId: number;
    status: string;
    contactType: string;
    contactValue: string | null;
    locale: string | null;
    metadata: string | null;
    unsubscribeToken: string | null;
    sectionId?: number | null;
    termId?: string | null;
    campusCode?: string | null;
    indexNumber?: string | null;
    lastKnownStatus?: string | null;
  };
  section?: SectionRow;
  course?: CourseRow;
  meetings: MeetingRow[];
};

export type DiscordAdapterOptions = {
  config: ResolvedDiscordBotConfig;
  appBaseUrl: string;
  defaultLocale: string;
  allowedChannelIds?: string[];
};

export type DiscordBuildResult =
  | { ok: true; request: DiscordSendRequest }
  | { ok: false; code: 'unsupported_contact' | 'invalid_target' | 'channel_blocked'; message: string };

export function buildDiscordSend(job: DiscordFanoutJob, opts: DiscordAdapterOptions): DiscordBuildResult {
  const contactType = job.subscription.contactType as DiscordContactType;
  if (contactType !== 'discord_user' && contactType !== 'discord_channel') {
    return { ok: false, code: 'unsupported_contact', message: `contact type ${job.subscription.contactType}` };
  }

  const metadata = parseMetadata(job.subscription.metadata);
  const target = resolveTarget(contactType, job.subscription.contactValue, metadata, opts.allowedChannelIds);
  if (!target.ok) {
    return target;
  }

  const subscriptionId = job.subscription.subscriptionId;
  const links = buildLinks(opts.appBaseUrl, subscriptionId, job.subscription.unsubscribeToken);
  const locale = chooseLocale(job.subscription.locale, opts.defaultLocale);
  const message = buildMessagePayload(job, opts.config, {
    manageUrl: links.manageUrl,
    unsubscribeUrl: links.unsubscribeUrl,
    locale,
  });

  return {
    ok: true,
    request: {
      target: target.target,
      message,
      traceId: job.event.traceId ?? undefined,
      dedupeKey: job.dedupeKey,
    },
  };
}

type ParsedMetadata = {
  preferences?: {
    channelMetadata?: Record<string, unknown>;
  };
  discord?: {
    guildId?: string;
    channelId?: string;
    userId?: string;
  };
};

function parseMetadata(raw: string | null): ParsedMetadata {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw) as ParsedMetadata;
    if (parsed && typeof parsed === 'object') {
      return parsed;
    }
  } catch {
    // best effort
  }
  return {};
}

type TargetResolution =
  | { ok: true; target: DiscordTarget }
  | { ok: false; code: 'invalid_target' | 'channel_blocked'; message: string };

function resolveTarget(
  contactType: DiscordContactType,
  contactValue: string | null,
  metadata: ParsedMetadata,
  allowedChannels?: string[],
): TargetResolution {
  if (contactType === 'discord_channel') {
    const sourceId = metadata.discord?.channelId ?? contactValue ?? null;
    if (!sourceId) {
      return { ok: false, code: 'invalid_target', message: 'missing channel id' };
    }
    if (Array.isArray(allowedChannels) && allowedChannels.length > 0 && !allowedChannels.includes(sourceId)) {
      return { ok: false, code: 'channel_blocked', message: `channel ${sourceId} not in allowlist` };
    }
    const guildId = metadata.discord?.guildId;
    return { ok: true, target: { channelId: sourceId, guildId: guildId ?? undefined } };
  }

  const userId = metadata.discord?.userId ?? contactValue ?? null;
  if (!userId) {
    return { ok: false, code: 'invalid_target', message: 'missing user id' };
  }
  return { ok: true, target: { userId, guildId: metadata.discord?.guildId ?? undefined } };
}

function buildMessagePayload(
  job: DiscordFanoutJob,
  config: ResolvedDiscordBotConfig,
  context: { manageUrl: string; unsubscribeUrl?: string; locale: string },
): DiscordMessagePayload {
  const courseTitle = resolveCourseTitle(job);
  const sectionNumber = resolveSectionNumber(job);
  const seatDelta = resolveSeatDelta(job);
  const meetingSummary = buildMeetingSummary(job.meetings);
  const socUrl = renderTemplate(config.messageTemplate.socUrlTemplate, {
    ...buildTemplateContext(job),
    manageUrl: context.manageUrl,
  });

  const templateContext = {
    ...buildTemplateContext(job),
    courseTitle,
    seatDelta,
    sectionNumber,
    meetingSummary,
    manageUrl: context.manageUrl,
    unsubscribeUrl: context.unsubscribeUrl,
    socUrl,
  };

  const prefix = renderTemplate(config.messageTemplate.prefix, templateContext);
  const statusLine =
    renderTemplate(config.messageTemplate.statusLine, templateContext) ??
    `${courseTitle} (${job.event.indexNumber} @ ${job.event.campusCode}) is OPEN`;
  const meetingLine =
    renderTemplate(config.messageTemplate.meetingLine, templateContext) ??
    (meetingSummary ? `When/Where: ${meetingSummary}` : null);
  const links: string[] = [];
  if (templateContext.manageUrl) {
    links.push(`Manage: ${templateContext.manageUrl}`);
  }
  if (templateContext.socUrl) {
    links.push(`SOC: ${templateContext.socUrl}`);
  }
  const linksLine = links.length ? links.join(' | ') : null;
  const footer = renderTemplate(config.messageTemplate.footer, templateContext);
  const traceLine = job.event.traceId ? `Trace: ${job.event.traceId}` : null;

  const lines = [prefix, statusLine, meetingLine, linksLine, traceLine, footer].filter(
    (line): line is string => Boolean(line && line.trim().length),
  );
  const content = lines.join('\n');

  return { content, allowedMentions: { parse: [] } };
}

function buildTemplateContext(job: DiscordFanoutJob) {
  return {
    termId: job.event.termId,
    campusCode: job.event.campusCode,
    indexNumber: job.event.indexNumber,
    statusAfter: job.event.statusAfter ?? 'OPEN',
    statusBefore: job.event.statusBefore ?? null,
    seatDelta: resolveSeatDelta(job),
    eventAt: job.event.eventAt,
    traceId: job.event.traceId ?? null,
  };
}

function resolveCourseTitle(job: DiscordFanoutJob): string {
  if (job.course?.title) return job.course.title;
  if (typeof job.event.payload.courseTitle === 'string') return job.event.payload.courseTitle;
  return 'Course update';
}

function resolveSectionNumber(job: DiscordFanoutJob): string | null {
  if (job.section?.section_number) return job.section.section_number;
  if (typeof job.event.payload.sectionNumber === 'string') return job.event.payload.sectionNumber;
  return null;
}

function resolveSeatDelta(job: DiscordFanoutJob): number | null {
  if (typeof job.event.seatDelta === 'number') return job.event.seatDelta;
  if (typeof job.event.payload.seatDelta === 'number') return job.event.payload.seatDelta;
  return null;
}

function buildMeetingSummary(meetings: MeetingRow[]): string {
  if (!meetings.length) return 'TBA';
  return meetings
    .map((meeting) => {
      const day = meeting.meeting_day ?? 'TBA';
      const time =
        meeting.start_minutes !== null && meeting.start_minutes !== undefined && meeting.end_minutes !== null
          ? `${formatMinutes(meeting.start_minutes)}-${formatMinutes(meeting.end_minutes)}`
          : null;
      const locationParts: string[] = [];
      if (meeting.campus_abbrev) locationParts.push(meeting.campus_abbrev);
      else if (meeting.campus_location_code) locationParts.push(meeting.campus_location_code);
      else if (meeting.campus_location_desc) locationParts.push(meeting.campus_location_desc);
      if (meeting.building_code) locationParts.push(meeting.building_code);
      if (meeting.room_number) locationParts.push(meeting.room_number);
      const location = locationParts.length ? locationParts.join(' ') : null;
      return [day, time, location].filter(Boolean).join(' ');
    })
    .join('; ');
}

function formatMinutes(value: number): string {
  const hours = Math.floor(value / 60);
  const minutes = value % 60;
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
}

function renderTemplate(template: string | undefined, context: Record<string, unknown>): string | null {
  if (!template) return null;
  return template.replace(/{{\s*([^}]+)\s*}}/g, (_, key: string) => {
    const value = context[key as keyof typeof context];
    return value === null || value === undefined ? '' : String(value);
  });
}

function chooseLocale(preferred: string | null, fallback: string): string {
  if (preferred && preferred.trim().length) return preferred;
  return fallback;
}

function buildLinks(base: string, subscriptionId: number, unsubscribeToken: string | null) {
  const normalized = base.replace(/\/+$/, '');
  const manageUrl = `${normalized}/subscriptions/${subscriptionId}`;
  const unsubscribeUrl = unsubscribeToken
    ? `${normalized}/unsubscribe?token=${encodeURIComponent(unsubscribeToken)}`
    : undefined;
  return { manageUrl, unsubscribeUrl };
}
