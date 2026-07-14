import type {
  WatchAudioCueV1,
  WatchCueOutcome,
} from '../product/contracts/watch';

export type WatchAudioUnlockResult = 'READY' | 'BLOCKED' | 'FAILED';

export interface WatchAudioParamPort {
  setValueAtTime(value: number, startTime: number): void;
  linearRampToValueAtTime(value: number, endTime: number): void;
  exponentialRampToValueAtTime(value: number, endTime: number): void;
}

export interface WatchAudioOscillatorPort {
  type: OscillatorType;
  readonly frequency: WatchAudioParamPort;
  onended: (() => void) | null;
  connect(destination: unknown): unknown;
  disconnect(): void;
  start(when?: number): void;
  stop(when?: number): void;
}

export interface WatchAudioGainPort {
  readonly gain: WatchAudioParamPort;
  connect(destination: unknown): unknown;
  disconnect(): void;
}

export interface WatchAudioContextPort {
  readonly currentTime: number;
  readonly destination: unknown;
  state: AudioContextState;
  close?(): Promise<void>;
  createGain(): WatchAudioGainPort;
  createOscillator(): WatchAudioOscillatorPort;
  resume(): Promise<void>;
}

export interface WatchAudioTimerPort {
  setInterval(callback: () => void, intervalMilliseconds: number): unknown;
  clearInterval(handle: unknown): void;
}

export interface WatchAudioControllerOptions {
  readonly createContext?: () => WatchAudioContextPort;
  readonly timers?: WatchAudioTimerPort;
  readonly continuousRepeatMilliseconds?: number;
}

interface ActiveVoice {
  readonly continuous: boolean;
  stop(): void;
}

const ONE_SHOT_DURATION_SECONDS = 0.28;
const CONTINUOUS_DURATION_SECONDS = 0.22;
const DEFAULT_CONTINUOUS_REPEAT_MILLISECONDS = 900;
const MINIMUM_GAIN = 0.0001;

const browserTimers: WatchAudioTimerPort = {
  setInterval(callback, intervalMilliseconds) {
    return globalThis.setInterval(callback, intervalMilliseconds);
  },
  clearInterval(handle) {
    globalThis.clearInterval(handle as ReturnType<typeof globalThis.setInterval>);
  },
};

/** Browser-only, session-scoped audio. Product watch state remains authoritative elsewhere. */
export class WatchAudioController {
  readonly #createContext: () => WatchAudioContextPort;
  readonly #timers: WatchAudioTimerPort;
  readonly #continuousRepeatMilliseconds: number;
  readonly #voices = new Set<ActiveVoice>();
  #context: WatchAudioContextPort | null = null;
  #continuousTimer: unknown | null = null;
  #continuousVolume = 1;
  #audioFailure = false;
  #disposed = false;

  constructor(options: WatchAudioControllerOptions = {}) {
    this.#createContext = options.createContext ?? createBrowserContext;
    this.#timers = options.timers ?? browserTimers;
    this.#continuousRepeatMilliseconds = positiveInterval(
      options.continuousRepeatMilliseconds ?? DEFAULT_CONTINUOUS_REPEAT_MILLISECONDS,
    );
  }

  async unlock(): Promise<WatchAudioUnlockResult> {
    if (this.#disposed) return 'FAILED';
    this.#audioFailure = false;
    let context = this.#context;
    if (context === null) {
      try {
        context = this.#createContext();
        this.#context = context;
      } catch {
        this.#audioFailure = true;
        return 'FAILED';
      }
    }
    if (context.state === 'running') return 'READY';
    if (context.state === 'closed') {
      this.#audioFailure = true;
      return 'FAILED';
    }
    try {
      await context.resume();
    } catch (error) {
      if (isAutoplayBlock(error)) return 'BLOCKED';
      this.#audioFailure = true;
      return 'FAILED';
    }
    return readContextState(context) === 'running' ? 'READY' : 'BLOCKED';
  }

  play(cue: WatchAudioCueV1, volume: number, muted: boolean): WatchCueOutcome {
    if (cue.trigger.kind === 'CONTINUOUS_EPISODE') {
      return this.startContinuous(volume, muted);
    }
    return this.#playOneShot(volume, muted);
  }

  /** Plays a local-only cue so the user can verify the selected output and volume. */
  preview(volume: number): WatchCueOutcome {
    return this.#playOneShot(volume, false);
  }

  #playOneShot(volume: number, muted: boolean): WatchCueOutcome {
    if (this.#disposed || !Number.isFinite(volume)) return 'FAILED';
    const normalizedVolume = Math.min(100, Math.max(0, volume)) / 100;
    if (muted || normalizedVolume === 0) {
      return 'MUTED';
    }
    const context = this.#context;
    if (context === null || context.state !== 'running') {
      return this.#audioFailure ? 'FAILED' : 'AUTOPLAY_BLOCKED';
    }
    try {
      this.#startVoice(context, normalizedVolume, false);
      return 'STARTED';
    } catch {
      this.#audioFailure = true;
      return 'FAILED';
    }
  }

  /** Mirrors the server's aggregate CONTINUOUS_MIXER_ACTIVE disposition. */
  startContinuous(volume: number, muted: boolean): WatchCueOutcome {
    if (this.#disposed || !Number.isFinite(volume)) return 'FAILED';
    const normalizedVolume = Math.min(100, Math.max(0, volume)) / 100;
    if (muted || normalizedVolume === 0) {
      this.stopContinuous();
      return 'MUTED';
    }
    const context = this.#context;
    if (context === null || context.state !== 'running') {
      this.stopContinuous();
      return this.#audioFailure ? 'FAILED' : 'AUTOPLAY_BLOCKED';
    }
    try {
      this.#startContinuous(context, normalizedVolume);
      return 'STARTED';
    } catch {
      this.#audioFailure = true;
      this.stopContinuous();
      return 'FAILED';
    }
  }

  stopContinuous(): void {
    if (this.#continuousTimer !== null) {
      this.#timers.clearInterval(this.#continuousTimer);
      this.#continuousTimer = null;
    }
    for (const voice of [...this.#voices]) {
      if (voice.continuous) voice.stop();
    }
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.stopContinuous();
    for (const voice of [...this.#voices]) voice.stop();
    const context = this.#context;
    this.#context = null;
    if (context?.close !== undefined) {
      try {
        void context.close().catch(() => undefined);
      } catch {
        // Disposal is best-effort after every active voice and timer is stopped.
      }
    }
  }

  #startContinuous(context: WatchAudioContextPort, volume: number): void {
    this.#continuousVolume = volume;
    if (this.#continuousTimer !== null) return;
    this.#startVoice(context, volume, true);
    this.#continuousTimer = this.#timers.setInterval(() => {
      if (this.#disposed || context.state !== 'running') {
        this.stopContinuous();
        return;
      }
      try {
        this.#startVoice(context, this.#continuousVolume, true);
      } catch {
        this.#audioFailure = true;
        this.stopContinuous();
      }
    }, this.#continuousRepeatMilliseconds);
  }

  #startVoice(context: WatchAudioContextPort, volume: number, continuous: boolean): void {
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    const startedAt = context.currentTime;
    const duration = continuous ? CONTINUOUS_DURATION_SECONDS : ONE_SHOT_DURATION_SECONDS;
    let stopped = false;
    let cleaned = false;
    const cleanup = () => {
      if (cleaned) return;
      cleaned = true;
      this.#voices.delete(voice);
      try {
        oscillator.disconnect();
      } catch {
        // The browser may already have disconnected a completed source.
      }
      try {
        gain.disconnect();
      } catch {
        // The browser may already have disconnected a completed source.
      }
    };
    const voice: ActiveVoice = {
      continuous,
      stop: () => {
        if (!stopped) {
          stopped = true;
          try {
            oscillator.stop(context.currentTime);
          } catch {
            // A naturally ended oscillator is already stopped.
          }
        }
        cleanup();
      },
    };
    oscillator.onended = cleanup;
    this.#voices.add(voice);
    try {
      oscillator.type = 'sine';
      oscillator.frequency.setValueAtTime(continuous ? 740 : 880, startedAt);
      oscillator.frequency.linearRampToValueAtTime(continuous ? 880 : 660, startedAt + duration);
      gain.gain.setValueAtTime(MINIMUM_GAIN, startedAt);
      gain.gain.linearRampToValueAtTime(Math.max(MINIMUM_GAIN, volume * 0.24), startedAt + 0.025);
      gain.gain.exponentialRampToValueAtTime(MINIMUM_GAIN, startedAt + duration);
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.start(startedAt);
      oscillator.stop(startedAt + duration);
    } catch (error) {
      voice.stop();
      throw error;
    }
  }
}

function createBrowserContext(): WatchAudioContextPort {
  return new globalThis.AudioContext() as unknown as WatchAudioContextPort;
}

function positiveInterval(value: number): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new RangeError('continuous repeat interval must be positive');
  }
  return value;
}

function readContextState(context: WatchAudioContextPort): AudioContextState {
  return context.state;
}

function isAutoplayBlock(error: unknown): boolean {
  return typeof error === 'object'
    && error !== null
    && 'name' in error
    && error.name === 'NotAllowedError';
}
