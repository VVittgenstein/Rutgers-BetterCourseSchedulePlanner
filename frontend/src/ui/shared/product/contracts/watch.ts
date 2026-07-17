import type {
  ContractVersionV1,
  IsoDateTime,
  SectionKey,
  TraceId,
} from './common';
import type { OpenObservationV1 } from './open';

export type ActiveWatchId = TraceId;
export type OpenEpisodeId = TraceId;
export type WatchAlertId = TraceId;
export type WatchCueId = TraceId;
export type WatchNotificationMode = 'ONE_SHOT' | 'CONTINUOUS';
export type WatchContinuousDurationV1 =
  | { readonly kind: 'FINITE'; readonly seconds: number }
  | { readonly kind: 'UNLIMITED' };

export interface WatchPolicyV1 {
  readonly notificationMode: WatchNotificationMode;
  readonly maxAudible: number;
  readonly continuousDuration: WatchContinuousDurationV1;
}

export interface WatchStartItemV1 {
  readonly sectionKey: SectionKey;
  readonly policy: WatchPolicyV1;
}

export interface ActiveWatchTargetV1 {
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
}

export interface WatchEpisodeTargetV1 extends ActiveWatchTargetV1 {
  readonly episodeId: OpenEpisodeId;
}

export interface WatchAlertTargetV1 extends WatchEpisodeTargetV1 {
  readonly alertId: WatchAlertId;
}

export type WatchCueOutcome = 'STARTED' | 'MUTED' | 'AUTOPLAY_BLOCKED' | 'FAILED';

export interface WatchCueOutcomeReportV1 {
  readonly cueId: WatchCueId;
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
  readonly outcome: WatchCueOutcome;
  readonly reportedAt: IsoDateTime;
}

export type WatchClientCommandV1 =
  | { readonly type: 'START_WATCH'; readonly items: readonly WatchStartItemV1[] }
  | { readonly type: 'STOP_WATCH'; readonly watch: ActiveWatchTargetV1 }
  | { readonly type: 'UPDATE_POLICY'; readonly watch: ActiveWatchTargetV1; readonly policy: WatchPolicyV1 }
  | { readonly type: 'ACKNOWLEDGE_EPISODE'; readonly episode: WatchEpisodeTargetV1 }
  | { readonly type: 'ACKNOWLEDGE_ALL_EPISODES' }
  | { readonly type: 'RESUME_TIMED_OUT_EPISODE'; readonly episode: WatchEpisodeTargetV1 }
  | { readonly type: 'RESET_AUDIBLE_COUNT'; readonly watch: ActiveWatchTargetV1 }
  | { readonly type: 'REPORT_CUE_OUTCOME'; readonly report: WatchCueOutcomeReportV1 }
  | { readonly type: 'DISMISS_ALERT'; readonly alert: WatchAlertTargetV1 };

export type WatchStartRejectionReason =
  | 'MAX_ACTIVE_WATCHES'
  | 'ALREADY_ACTIVE'
  | 'SECTION_NOT_FOUND'
  | 'TARGET_UNAVAILABLE'
  | 'TERM_OUT_OF_RANGE'
  | 'CONNECTION_CLOSING';
export type WatchStartItemResultV1 =
  | {
      readonly status: 'ACTIVE';
      readonly sectionKey: SectionKey;
      readonly activeWatchId: ActiveWatchId;
      readonly startedAt: IsoDateTime;
    }
  | {
      readonly status: 'REJECTED';
      readonly sectionKey: SectionKey;
      readonly reason: WatchStartRejectionReason;
    };

export interface WatchStartResultV1 {
  readonly contractVersion: ContractVersionV1;
  readonly items: readonly WatchStartItemResultV1[];
  readonly activeWatchCount: number;
}

export interface WatchStoppedV1 {
  readonly contractVersion: ContractVersionV1;
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
  readonly reason: 'USER_REQUESTED' | 'CONNECTION_CLOSED' | 'HEARTBEAT_TIMEOUT' | 'SERVICE_STOPPING' | 'TERM_OUT_OF_RANGE';
  readonly stoppedAt: IsoDateTime;
}

export interface WatchOpenObservationFanoutV1 {
  readonly contractVersion: ContractVersionV1;
  readonly activeWatchId: ActiveWatchId;
  readonly observation: OpenObservationV1;
}

export type OpenEpisodeState = 'UNACKNOWLEDGED' | 'ACKNOWLEDGED' | 'TIMED_OUT' | 'CLOSED';

export interface OpenEpisodeV1 {
  readonly contractVersion: ContractVersionV1;
  readonly episodeId: OpenEpisodeId;
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
  readonly state: OpenEpisodeState;
  readonly notificationMode: WatchNotificationMode;
  readonly continuousDuration: WatchContinuousDurationV1;
  readonly maxAudible: number;
  readonly audibleCount: number;
  readonly firstObservedAt: IsoDateTime;
  readonly lastObservedAt: IsoDateTime;
  readonly observationCount: number;
  readonly latestObservationId: TraceId;
  readonly stateChangedAt: IsoDateTime;
  readonly closedAt?: IsoDateTime | null;
}

export interface WatchAlertV1 {
  readonly contractVersion: ContractVersionV1;
  readonly alertId: WatchAlertId;
  readonly disposition: 'OPENED' | 'UPDATED' | 'DISMISSED' | 'CLOSED';
  readonly visible: boolean;
  readonly episode: OpenEpisodeV1;
}

export type WatchAudioTriggerV1 =
  | { readonly kind: 'ONE_SHOT_OBSERVATION'; readonly observationId: TraceId }
  | { readonly kind: 'CONTINUOUS_EPISODE'; readonly episodeId: OpenEpisodeId };

export interface WatchAudioCueV1 {
  readonly cueId: WatchCueId;
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
  readonly trigger: WatchAudioTriggerV1;
  readonly emittedAt: IsoDateTime;
}

export type WatchAudioDispositionV1 =
  | { readonly disposition: 'CUE_REQUESTED'; readonly cue: WatchAudioCueV1 }
  | {
      readonly disposition: 'CUE_QUEUED';
      readonly activeWatchId: ActiveWatchId;
      readonly sectionKey: SectionKey;
      readonly observationId: TraceId;
      readonly queuedAt: IsoDateTime;
    }
  | {
      readonly disposition: 'CUE_CANCELLED';
      readonly activeWatchId: ActiveWatchId;
      readonly sectionKey: SectionKey;
      readonly observationId: TraceId;
      readonly cueId: WatchCueId | null;
      readonly reason: 'WATCH_STOPPED' | 'WATCH_RESTARTED' | 'NOTIFICATION_MODE_CHANGED' | 'CONNECTION_CLOSED';
      readonly cancelledAt: IsoDateTime;
    }
  | {
      readonly disposition: 'SILENT_MAX_AUDIBLE';
      readonly activeWatchId: ActiveWatchId;
      readonly sectionKey: SectionKey;
      readonly observationId: TraceId;
      readonly audibleCount: number;
      readonly maxAudible: number;
      readonly emittedAt: IsoDateTime;
    }
  | {
      readonly disposition: 'CONTINUOUS_MIXER_ACTIVE';
      readonly episodeIds: readonly OpenEpisodeId[];
      readonly emittedAt: IsoDateTime;
    }
  | {
      readonly disposition: 'CONTINUOUS_MIXER_STOPPED';
      readonly reason: 'NO_UNACKNOWLEDGED_EPISODES' | 'NOTIFICATION_MODE_CHANGED' | 'CONNECTION_CLOSED' | 'SERVICE_STOPPING';
      readonly emittedAt: IsoDateTime;
    };

export interface WatchCueOutcomeReceiptV1 {
  readonly contractVersion: ContractVersionV1;
  readonly cueId: WatchCueId;
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
  readonly outcome: WatchCueOutcome;
  readonly audibleCountConsumed: boolean;
  readonly audibleCount: number;
  readonly recordedAt: IsoDateTime;
}

export type WatchServerEventV1 =
  | { readonly type: 'START_RESULT'; readonly result: WatchStartResultV1 }
  | { readonly type: 'WATCH_STOPPED'; readonly stopped: WatchStoppedV1 }
  | { readonly type: 'OPEN_OBSERVATION'; readonly fanout: WatchOpenObservationFanoutV1 }
  | { readonly type: 'EPISODE_UPDATED'; readonly episode: OpenEpisodeV1 }
  | { readonly type: 'ALERT_UPDATED'; readonly alert: WatchAlertV1 }
  | { readonly type: 'AUDIO_DISPOSITION'; readonly audio: WatchAudioDispositionV1 }
  | { readonly type: 'CUE_OUTCOME_RECORDED'; readonly receipt: WatchCueOutcomeReceiptV1 };
