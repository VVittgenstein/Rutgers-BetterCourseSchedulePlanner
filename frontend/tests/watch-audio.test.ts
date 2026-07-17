import { describe, expect, it, vi } from 'vitest';

import type { WatchAudioCueV1 } from '../src/ui/shared/product/contracts/watch';
import {
  WatchAudioController,
  type WatchAudioContextPort,
  type WatchAudioGainPort,
  type WatchAudioOscillatorPort,
  type WatchAudioParamPort,
  type WatchAudioTimerPort,
} from '../src/ui/shared/watch/audio';

const ONE_SHOT_CUE: WatchAudioCueV1 = {
  cueId: '00000000-0000-4000-8000-000000000001',
  activeWatchId: '00000000-0000-4000-8000-000000000002',
  sectionKey: { term: 'T2026F', campus: 'CAMPUS_A', index: '00001' },
  trigger: {
    kind: 'ONE_SHOT_OBSERVATION',
    observationId: '00000000-0000-4000-8000-000000000003',
  },
  emittedAt: '1970-01-01T00:00:00Z',
};

const CONTINUOUS_CUE: WatchAudioCueV1 = {
  ...ONE_SHOT_CUE,
  cueId: '00000000-0000-4000-8000-000000000004',
  trigger: {
    kind: 'CONTINUOUS_EPISODE',
    episodeId: '00000000-0000-4000-8000-000000000005',
  },
};

class FakeParam implements WatchAudioParamPort {
  readonly changes: Array<readonly [string, number, number]> = [];

  setValueAtTime(value: number, time: number): void {
    this.changes.push(['set', value, time]);
  }

  linearRampToValueAtTime(value: number, time: number): void {
    this.changes.push(['linear', value, time]);
  }

  exponentialRampToValueAtTime(value: number, time: number): void {
    this.changes.push(['exponential', value, time]);
  }
}

class FakeOscillator implements WatchAudioOscillatorPort {
  type: OscillatorType = 'sine';
  readonly frequency = new FakeParam();
  onended: (() => void) | null = null;
  readonly start = vi.fn<(when?: number) => void>();
  readonly stop = vi.fn<(when?: number) => void>();
  readonly connect = vi.fn<(destination: unknown) => unknown>((destination) => destination);
  readonly disconnect = vi.fn<() => void>();
}

class FakeGain implements WatchAudioGainPort {
  readonly gain = new FakeParam();
  readonly connect = vi.fn<(destination: unknown) => unknown>((destination) => destination);
  readonly disconnect = vi.fn<() => void>();
}

class FakeContext implements WatchAudioContextPort {
  currentTime = 4;
  destination: unknown = { output: true };
  state: AudioContextState = 'suspended';
  readonly oscillators: FakeOscillator[] = [];
  readonly gains: FakeGain[] = [];
  readonly resume = vi.fn(async () => {
    this.state = 'running';
  });
  readonly close = vi.fn(async () => {
    this.state = 'closed';
  });
  failOscillator = false;

  createGain(): FakeGain {
    const gain = new FakeGain();
    this.gains.push(gain);
    return gain;
  }

  createOscillator(): FakeOscillator {
    if (this.failOscillator) throw new Error('oscillator unavailable');
    const oscillator = new FakeOscillator();
    this.oscillators.push(oscillator);
    return oscillator;
  }
}

class FakeTimers implements WatchAudioTimerPort {
  readonly callbacks = new Map<number, () => void>();
  readonly cleared: number[] = [];
  #next = 1;

  setInterval(callback: () => void): number {
    const handle = this.#next;
    this.#next += 1;
    this.callbacks.set(handle, callback);
    return handle;
  }

  clearInterval(handle: unknown): void {
    const numeric = Number(handle);
    this.callbacks.delete(numeric);
    this.cleared.push(numeric);
  }

  tick(): void {
    for (const callback of [...this.callbacks.values()]) callback();
  }
}

describe('WatchAudioController', () => {
  it('unlocks a suspended context and distinguishes autoplay blocking from failure', async () => {
    const ready = new FakeContext();
    const controller = new WatchAudioController({ createContext: () => ready });
    await expect(controller.unlock()).resolves.toBe('READY');
    expect(ready.resume).toHaveBeenCalledOnce();

    const blocked = new FakeContext();
    blocked.resume.mockImplementationOnce(async () => {
      throw new DOMException('gesture required', 'NotAllowedError');
    });
    await expect(new WatchAudioController({ createContext: () => blocked }).unlock())
      .resolves.toBe('BLOCKED');

    const failed = new FakeContext();
    failed.resume.mockRejectedValueOnce(new Error('device failure'));
    await expect(new WatchAudioController({ createContext: () => failed }).unlock())
      .resolves.toBe('FAILED');
    await expect(new WatchAudioController({ createContext: () => {
      throw new Error('no audio device');
    } }).unlock()).resolves.toBe('FAILED');
  });

  it('plays one short cue only after unlock and reports muted, blocked, and failed outcomes', async () => {
    const context = new FakeContext();
    const controller = new WatchAudioController({ createContext: () => context });
    expect(controller.play(ONE_SHOT_CUE, 50, false)).toBe('AUTOPLAY_BLOCKED');
    expect(controller.play(ONE_SHOT_CUE, 50, true)).toBe('MUTED');
    expect(controller.play(ONE_SHOT_CUE, 0, false)).toBe('MUTED');

    await expect(controller.unlock()).resolves.toBe('READY');
    expect(controller.play(ONE_SHOT_CUE, 50, false)).toBe('STARTED');
    expect(context.oscillators).toHaveLength(1);
    expect(context.oscillators[0]?.start).toHaveBeenCalledWith(4);
    expect(context.oscillators[0]?.stop).toHaveBeenCalledWith(4.28);
    expect(context.gains[0]?.gain.changes).toContainEqual(['linear', 0.12, 4.025]);

    context.failOscillator = true;
    expect(controller.play(ONE_SHOT_CUE, 50, false)).toBe('FAILED');
    expect(controller.play(ONE_SHOT_CUE, Number.NaN, false)).toBe('FAILED');
  });

  it('plays an explicit local preview through the configured output and volume', async () => {
    const context = new FakeContext();
    const controller = new WatchAudioController({ createContext: () => context });
    expect(controller.preview(50)).toBe('AUTOPLAY_BLOCKED');
    expect(controller.preview(0)).toBe('MUTED');

    await expect(controller.unlock()).resolves.toBe('READY');
    expect(controller.preview(50)).toBe('STARTED');
    expect(context.oscillators).toHaveLength(1);
    expect(context.oscillators[0]?.start).toHaveBeenCalledWith(4);
    expect(context.oscillators[0]?.stop).toHaveBeenCalledWith(4.28);
    expect(context.gains[0]?.gain.changes).toContainEqual(['linear', 0.12, 4.025]);

    context.failOscillator = true;
    expect(controller.preview(50)).toBe('FAILED');
  });

  it('uses one aggregate continuous timer and reliably stops every continuous voice', async () => {
    const context = new FakeContext();
    const timers = new FakeTimers();
    const controller = new WatchAudioController({
      createContext: () => context,
      timers,
      continuousRepeatMilliseconds: 25,
    });
    await controller.unlock();

    expect(controller.startContinuous(80, false)).toBe('STARTED');
    expect(context.oscillators).toHaveLength(1);
    expect(timers.callbacks.size).toBe(1);
    expect(controller.startContinuous(40, false)).toBe('STARTED');
    expect(context.oscillators).toHaveLength(1);
    expect(timers.callbacks.size).toBe(1);

    timers.tick();
    expect(context.oscillators).toHaveLength(2);
    expect(context.gains[1]?.gain.changes).toContainEqual(['linear', 0.096, 4.025]);
    controller.stopContinuous();
    expect(timers.callbacks.size).toBe(0);
    expect(timers.cleared).toEqual([1]);
    expect(context.oscillators.every((oscillator) => oscillator.disconnect.mock.calls.length > 0))
      .toBe(true);
    const countAfterStop = context.oscillators.length;
    timers.tick();
    expect(context.oscillators).toHaveLength(countAfterStop);
  });

  it('stops a continuous cue when audio becomes suspended or muted and disposes idempotently', async () => {
    const context = new FakeContext();
    const timers = new FakeTimers();
    const controller = new WatchAudioController({ createContext: () => context, timers });
    await controller.unlock();
    expect(controller.startContinuous(100, false)).toBe('STARTED');
    context.state = 'suspended';
    timers.tick();
    expect(timers.callbacks.size).toBe(0);
    expect(controller.startContinuous(100, false)).toBe('AUTOPLAY_BLOCKED');

    context.state = 'running';
    expect(controller.startContinuous(100, false)).toBe('STARTED');
    expect(controller.startContinuous(100, true)).toBe('MUTED');
    expect(timers.callbacks.size).toBe(0);

    controller.dispose();
    controller.dispose();
    expect(context.close).toHaveBeenCalledOnce();
    expect(controller.play(ONE_SHOT_CUE, 100, false)).toBe('FAILED');
    await expect(controller.unlock()).resolves.toBe('FAILED');
  });

  it('rejects a non-positive continuous timer configuration', () => {
    expect(() => new WatchAudioController({ continuousRepeatMilliseconds: 0 }))
      .toThrow(/must be positive/u);
  });
});
