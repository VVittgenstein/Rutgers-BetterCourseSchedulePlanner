import { describe, expect, it, vi } from 'vitest';

import {
  WatchAudioController,
  type WatchAudioContextPort,
  type WatchAudioGainPort,
  type WatchAudioOscillatorPort,
  type WatchAudioParamPort,
  type WatchAudioUnlockResult,
} from '../src/ui/shared/watch/audio';

class SilentParam implements WatchAudioParamPort {
  setValueAtTime(): void {}
  linearRampToValueAtTime(): void {}
  exponentialRampToValueAtTime(): void {}
}

class SilentOscillator implements WatchAudioOscillatorPort {
  type: OscillatorType = 'sine';
  readonly frequency = new SilentParam();
  onended: (() => void) | null = null;
  connect(destination: unknown): unknown { return destination; }
  disconnect(): void {}
  start(): void {}
  stop(): void {}
}

class SilentGain implements WatchAudioGainPort {
  readonly gain = new SilentParam();
  connect(destination: unknown): unknown { return destination; }
  disconnect(): void {}
}

function autoplayBlock(): Error {
  const error = new Error('gesture required');
  error.name = 'NotAllowedError';
  return error;
}

/**
 * A context the test suspends and resumes the way a laptop lid does.
 *
 * `statechange` is emitted by hand rather than by the setter, so a test can
 * also model the browser that suspends a context WITHOUT telling the page --
 * which is the case the page-driven recheck exists for.
 */
class SuspendableContext implements WatchAudioContextPort {
  currentTime = 0;
  destination: unknown = { output: true };
  state: AudioContextState = 'suspended';
  resumeError: Error | null = null;
  resumesTo: AudioContextState = 'running';
  readonly resume = vi.fn(async (): Promise<void> => {
    if (this.resumeError !== null) throw this.resumeError;
    this.state = this.resumesTo;
    if (this.resumesTo === 'running') this.emit();
  });
  readonly listeners = new Set<() => void>();

  addEventListener(_type: 'statechange', listener: () => void): void {
    this.listeners.add(listener);
  }

  createGain(): WatchAudioGainPort { return new SilentGain(); }
  createOscillator(): WatchAudioOscillatorPort { return new SilentOscillator(); }

  /** The system suspends the context and the browser announces it. */
  suspendAndAnnounce(): void {
    this.state = 'suspended';
    this.emit();
  }

  /** The system suspends the context while the page is away: no announcement. */
  suspendSilently(): void {
    this.state = 'suspended';
  }

  emit(): void {
    this.listeners.forEach((listener) => listener());
  }
}

async function unlocked(context: SuspendableContext): Promise<{
  readonly controller: WatchAudioController;
  readonly states: WatchAudioUnlockResult[];
}> {
  const controller = new WatchAudioController({ createContext: () => context });
  const states: WatchAudioUnlockResult[] = [];
  controller.subscribeState((state) => states.push(state));
  expect(await controller.unlock()).toBe('READY');
  states.length = 0;
  return { controller, states };
}

describe('WatchAudioController self-heal', () => {
  it('resumes a context the system suspended and reports the recovery', async () => {
    const context = new SuspendableContext();
    const { controller, states } = await unlocked(context);
    context.resume.mockClear();

    context.suspendAndAnnounce();
    // The listener repairs asynchronously; let its resume settle.
    await vi.waitFor(() => expect(context.state).toBe('running'));

    expect(context.resume).toHaveBeenCalledTimes(1);
    expect(controller.state).toBe('READY');
    // BLOCKED then READY, in that order: the page is told the truth on the
    // way down as well as on the way back, so a cue that lands in between is
    // not reported as delivered. The exact tail is not pinned because the
    // resume's own statechange re-enters heal and may publish READY twice.
    expect(states[0]).toBe('BLOCKED');
    expect(states.at(-1)).toBe('READY');
  });

  it('revokes READY the moment a suspend is announced, before the resume settles', async () => {
    const context = new SuspendableContext();
    const { controller, states } = await unlocked(context);
    let release: (() => void) | null = null;
    context.resume.mockImplementation(async () => {
      await new Promise<void>((resolve) => { release = resolve; });
      context.state = 'running';
    });

    context.suspendAndAnnounce();
    // The resume is still pending -- the browser may never settle it. The
    // page has to have been told already, not when (or if) the promise ends.
    expect(states).toContain('BLOCKED');
    expect(states).not.toContain('READY');
    expect(controller.state).toBe('BLOCKED');

    release?.();
    await vi.waitFor(() => expect(states.at(-1)).toBe('READY'));
    expect(controller.state).toBe('READY');
  });

  it('reports a blocked context honestly and stops asking until a gesture', async () => {
    const context = new SuspendableContext();
    const { controller } = await unlocked(context);
    context.resumeError = autoplayBlock();
    context.resume.mockClear();

    context.suspendAndAnnounce();
    await vi.waitFor(() => expect(context.resume).toHaveBeenCalledTimes(1));
    expect(controller.state).toBe('BLOCKED');
    expect(controller.gestureRequired).toBe(true);

    // Further state changes must not turn into a resume storm against a
    // browser that has already given its answer.
    context.emit();
    context.emit();
    await Promise.resolve();
    expect(context.resume).toHaveBeenCalledTimes(1);

    // The user presses the recovery control. That is a gesture, so the
    // question is asked again -- and this time the browser says yes.
    context.resumeError = null;
    expect(await controller.unlock()).toBe('READY');
    expect(controller.gestureRequired).toBe(false);
    expect(context.resume).toHaveBeenCalledTimes(2);
  });

  it('recovers a context that was suspended without any announcement', async () => {
    const context = new SuspendableContext();
    const { controller } = await unlocked(context);
    context.resume.mockClear();

    // The system suspended it while the page was hidden and said nothing. A
    // controller that only listened would still be claiming READY.
    context.suspendSilently();
    expect(controller.state).toBe('BLOCKED');

    expect(await controller.heal()).toBe('READY');
    expect(context.resume).toHaveBeenCalledTimes(1);
    expect(controller.state).toBe('READY');
  });

  it('reports a closed context as failed and never tries to resume it', async () => {
    const context = new SuspendableContext();
    const { controller } = await unlocked(context);
    context.resume.mockClear();

    context.state = 'closed';
    expect(await controller.heal()).toBe('FAILED');
    expect(context.resume).not.toHaveBeenCalled();
    expect(controller.state).toBe('FAILED');
  });

  it('says nothing about audio before anything has been unlocked', async () => {
    const context = new SuspendableContext();
    const controller = new WatchAudioController({ createContext: () => context });

    // No context exists yet, so there is no state to report and nothing to
    // repair. A controller that answered BLOCKED here would make a page that
    // has never asked for sound look broken.
    expect(controller.state).toBeNull();
    expect(await controller.heal()).toBeNull();
    expect(context.resume).not.toHaveBeenCalled();
  });

  it('does not run two repairs at once', async () => {
    const context = new SuspendableContext();
    const { controller } = await unlocked(context);
    context.resume.mockClear();
    let release: (() => void) | null = null;
    context.resume.mockImplementation(async () => {
      await new Promise<void>((resolve) => { release = resolve; });
      context.state = 'running';
    });

    context.suspendSilently();
    const first = controller.heal();
    const second = controller.heal();
    release?.();
    await Promise.all([first, second]);

    expect(context.resume).toHaveBeenCalledTimes(1);
  });
});
