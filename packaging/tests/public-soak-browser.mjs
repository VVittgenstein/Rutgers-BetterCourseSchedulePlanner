// P2 hardening H9: the browser half of the 600-second public soak.
//
// A real Chromium page, served by the real binary through the real Caddy,
// opens ONE watch WebSocket and holds it for the whole soak: every
// application PING must arrive on a strictly continuous sequence (a
// replacement connection restarts at 1, so continuity IS identity), every
// PING is acknowledged, and the connection must survive the mid-soak
// `caddy reload` the shell harness performs. The shell samples MemoryCurrent
// and the connection gauge; this script judges those samples afterwards via
// --analyze-memory / --analyze-connections so the arithmetic is testable.
//
// Modes:
//   (soak)                node public-soak-browser.mjs --base-url URL \
//                           --playwright-root PATH --armed-marker FILE \
//                           --reload-marker FILE [--done-marker FILE] \
//                           [--duration-seconds 600] [--expected-pings 50]
//   --analyze-memory F    judge one-MemoryCurrent-bytes-per-line samples
//   --analyze-connections F  judge one-connection-count-per-line samples
//   --self-test           prove the pure judgments on fixed fixtures
//
// The soak needs Playwright (borrowed from --playwright-root); the analyze
// and self-test modes are dependency-free so they run on any platform.

import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

export const MEMORY_LIMIT_BYTES = 700 * 1024 * 1024;
export const MEMORY_GROWTH_BUDGET_BYTES = 32 * 1024 * 1024;
export const MEMORY_WINDOW = 3;
export const DEFAULT_DURATION_SECONDS = 600;
export const DEFAULT_EXPECTED_PINGS = 50;

/** Judges the 30-second MemoryCurrent samples (bytes, one per line). */
export function analyzeMemorySamples(samples) {
  assert.ok(
    samples.length >= MEMORY_WINDOW * 2,
    `need at least ${MEMORY_WINDOW * 2} memory samples, got ${samples.length}`,
  );
  for (const sample of samples) {
    assert.ok(
      Number.isSafeInteger(sample) && sample > 0,
      `memory samples must be positive byte counts, got ${sample}`,
    );
    assert.ok(
      sample < MEMORY_LIMIT_BYTES,
      `memory sample ${sample} reached the 700 MiB ceiling (${MEMORY_LIMIT_BYTES})`,
    );
  }
  const average = (window) => window.reduce((sum, value) => sum + value, 0) / window.length;
  const firstAverage = average(samples.slice(0, MEMORY_WINDOW));
  const lastAverage = average(samples.slice(-MEMORY_WINDOW));
  const growth = lastAverage - firstAverage;
  assert.ok(
    growth <= MEMORY_GROWTH_BUDGET_BYTES,
    `memory grew ${Math.round(growth)} bytes between the first and last three samples; the budget is ${MEMORY_GROWTH_BUDGET_BYTES}`,
  );
  return { samples: samples.length, firstAverage, lastAverage, growth };
}

/** Judges the connection-gauge samples: the held socket must stay visible. */
export function analyzeConnectionSamples(samples) {
  assert.ok(samples.length > 0, 'no connection samples were recorded');
  for (const sample of samples) {
    assert.ok(
      Number.isSafeInteger(sample) && sample >= 1,
      `every connection sample must stay >= 1 while the soak socket is held, got ${sample}`,
    );
  }
  return { samples: samples.length };
}

/**
 * The identity judgment: application PING sequences are per-connection and
 * start at 1, so the one held socket must observe exactly 1, 2, 3, ... A
 * repeated or restarted sequence is a replacement connection; a gap is a
 * lost frame. Both fail.
 */
export function assertPingContinuity(sequences, expectedMinimum) {
  assert.ok(
    sequences.length >= expectedMinimum,
    `observed ${sequences.length} application pings; the soak requires at least ${expectedMinimum}`,
  );
  sequences.forEach((sequence, index) => {
    assert.equal(
      sequence,
      index + 1,
      `ping sequence must be continuous from 1 (position ${index} held ${sequence}); a restart here is a replacement connection`,
    );
  });
  return { pings: sequences.length };
}

/** Pings observed after the reload marker prove the same socket crossed it. */
export function assertReloadCrossing(sequences, sequenceAtReload) {
  assert.ok(
    sequenceAtReload >= 1,
    'the reload happened before the first ping; the soak cannot vouch for it',
  );
  const last = sequences[sequences.length - 1] ?? 0;
  assert.ok(
    last >= sequenceAtReload + 2,
    `only ${last - sequenceAtReload} ping(s) arrived after the reload (sequence ${sequenceAtReload}); the same socket must keep flowing`,
  );
  return { sequenceAtReload, afterReload: last - sequenceAtReload };
}

function readSampleFile(path) {
  return readFileSync(path, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '')
    .map((line) => {
      const value = Number.parseInt(line, 10);
      assert.ok(Number.isSafeInteger(value), `unparsable sample line: ${line}`);
      return value;
    });
}

function selfTest() {
  const mib = 1024 * 1024;
  // Memory: flat usage passes; the documented failures each refuse.
  const flat = Array.from({ length: 20 }, (_, index) => 400 * mib + (index % 3) * mib);
  analyzeMemorySamples(flat);
  assert.throws(
    () => analyzeMemorySamples(flat.map((value, index) => (index === 10 ? 700 * mib : value))),
    /700 MiB ceiling/,
    'a single sample at the ceiling must fail',
  );
  assert.throws(
    () =>
      analyzeMemorySamples(
        flat.map((value, index) => (index >= flat.length - 3 ? value + 33 * mib : value)),
      ),
    /budget is/,
    'last-three growth past 32 MiB must fail',
  );
  assert.throws(() => analyzeMemorySamples([400 * mib]), /at least/, 'too few samples must fail');
  // Growth exactly at the budget passes: the contract says "not more than".
  analyzeMemorySamples(
    [400 * mib, 400 * mib, 400 * mib, 400 * mib, 432 * mib, 432 * mib, 432 * mib],
  );

  // Connections: the gauge dropping to zero mid-soak is a lost socket.
  analyzeConnectionSamples([1, 1, 2, 1]);
  assert.throws(() => analyzeConnectionSamples([1, 0, 1]), /stay >= 1/);
  assert.throws(() => analyzeConnectionSamples([]), /no connection samples/);

  // Ping continuity: a restart, a gap, and a shortfall each refuse.
  assertPingContinuity(Array.from({ length: 60 }, (_, index) => index + 1), 50);
  assert.throws(
    () => assertPingContinuity([1, 2, 3, 1, 2], 3),
    /replacement connection/,
    'a sequence restart is a replaced socket',
  );
  assert.throws(() => assertPingContinuity([1, 2, 4], 3), /continuous from 1/);
  assert.throws(() => assertPingContinuity([1, 2], 50), /requires at least/);

  // Reload crossing: pings must continue on the same socket afterwards.
  assertReloadCrossing(Array.from({ length: 40 }, (_, index) => index + 1), 20);
  assert.throws(() => assertReloadCrossing([1, 2, 3], 3), /keep flowing/);
  assert.throws(() => assertReloadCrossing([1, 2, 3], 0), /before the first ping/);

  process.stdout.write('public-soak-browser self-test: PASS\n');
}

function parseArguments(argv) {
  const options = {
    baseUrl: null,
    playwrightRoot: null,
    armedMarker: null,
    reloadMarker: null,
    doneMarker: null,
    durationSeconds: DEFAULT_DURATION_SECONDS,
    expectedPings: DEFAULT_EXPECTED_PINGS,
    analyzeMemory: null,
    analyzeConnections: null,
    selfTest: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    const next = () => {
      index += 1;
      assert.ok(index < argv.length, `${flag} needs a value`);
      return argv[index];
    };
    switch (flag) {
      case '--base-url':
        options.baseUrl = next();
        break;
      case '--playwright-root':
        options.playwrightRoot = next();
        break;
      case '--armed-marker':
        options.armedMarker = next();
        break;
      case '--reload-marker':
        options.reloadMarker = next();
        break;
      case '--done-marker':
        options.doneMarker = next();
        break;
      case '--duration-seconds':
        options.durationSeconds = Number.parseInt(next(), 10);
        break;
      case '--expected-pings':
        options.expectedPings = Number.parseInt(next(), 10);
        break;
      case '--analyze-memory':
        options.analyzeMemory = next();
        break;
      case '--analyze-connections':
        options.analyzeConnections = next();
        break;
      case '--self-test':
        options.selfTest = true;
        break;
      default:
        assert.fail(`unknown argument: ${flag}`);
    }
  }
  return options;
}

const sleep = (milliseconds) => new Promise((resolveSleep) => setTimeout(resolveSleep, milliseconds));

async function runSoak(options) {
  assert.ok(options.baseUrl, '--base-url is required');
  assert.ok(options.playwrightRoot, '--playwright-root is required');
  assert.ok(options.armedMarker, '--armed-marker is required');
  assert.ok(options.reloadMarker, '--reload-marker is required');
  assert.ok(
    Number.isSafeInteger(options.durationSeconds) && options.durationSeconds >= 60,
    '--duration-seconds must be at least 60',
  );
  const parsedBase = new URL(options.baseUrl);
  assert.equal(parsedBase.protocol, 'https:', 'the soak runs through the real HTTPS edge');
  assert.equal(parsedBase.pathname, '/', 'pass the bare origin as --base-url');

  const requireFromFrontend = createRequire(resolve(options.playwrightRoot, 'package.json'));
  const { chromium } = requireFromFrontend('playwright');
  const launchArguments = ['--no-proxy-server'];
  if (typeof process.getuid === 'function' && process.getuid() === 0) {
    // The harness runs as root on a disposable host; Chromium refuses its
    // sandbox there (same accommodation as final-candidate-browser.mjs).
    launchArguments.push('--no-sandbox');
  }
  const browser = await chromium.launch({ args: launchArguments, headless: true });
  let failure = null;
  try {
    const context = await browser.newContext({
      locale: 'en-US',
      reducedMotion: 'reduce',
      colorScheme: 'light',
    });
    const page = await context.newPage();
    const socketClosures = [];
    page.on('websocket', (socket) => {
      socket.on('close', () => socketClosures.push(socket.url()));
    });
    const consoleErrors = [];
    page.on('pageerror', (error) => consoleErrors.push(String(error)));

    const response = await page.goto(options.baseUrl, { waitUntil: 'load', timeout: 30_000 });
    assert.ok(response?.ok(), `the document request failed: ${response?.status()}`);
    const nonce = await page.evaluate(() => {
      const bootstrap = document.getElementById('bcsp-bootstrap');
      if (!bootstrap) {
        return null;
      }
      return JSON.parse(bootstrap.textContent)?.data?.sessionNonce ?? null;
    });
    assert.ok(nonce, 'the served document must carry a session nonce');

    const socketUrl = `wss://${parsedBase.host}/api/v1/watch?session=${nonce}`;
    await page.evaluate((url) => {
      window.__soak = {
        opened: false,
        closed: null,
        errors: 0,
        pings: [],
        acks: 0,
        undecodable: 0,
      };
      const socket = new WebSocket(url, ['bcsp.v1']);
      window.__soakSocket = socket;
      socket.addEventListener('open', () => {
        window.__soak.opened = true;
      });
      socket.addEventListener('close', (event) => {
        window.__soak.closed = { code: event.code, reason: event.reason };
      });
      socket.addEventListener('error', () => {
        window.__soak.errors += 1;
      });
      socket.addEventListener('message', (event) => {
        try {
          const envelope = JSON.parse(event.data);
          if (envelope?.payload?.type === 'PING') {
            window.__soak.pings.push(envelope.payload.sequence);
            socket.send(
              JSON.stringify({
                protocolVersion: 1,
                messageId: crypto.randomUUID(),
                payload: { type: 'HEARTBEAT_ACK', sequence: envelope.payload.sequence },
              }),
            );
            window.__soak.acks += 1;
          }
        } catch {
          window.__soak.undecodable += 1;
        }
      });
    }, socketUrl);

    const soakState = () =>
      page.evaluate(() => ({
        ...window.__soak,
        readyState: window.__soakSocket?.readyState ?? -1,
      }));

    const openDeadline = Date.now() + 15_000;
    let state = await soakState();
    while (!state.opened && Date.now() < openDeadline) {
      await sleep(250);
      state = await soakState();
    }
    assert.ok(state.opened, 'the soak WebSocket did not open');
    writeFileSync(options.armedMarker, `${new Date().toISOString()}\n`);
    process.stdout.write(`public-soak: armed, holding one socket for ${options.durationSeconds}s\n`);

    const soakEnd = Date.now() + options.durationSeconds * 1_000;
    let sequenceAtReload = 0;
    let reloadObserved = false;
    while (Date.now() < soakEnd) {
      await sleep(5_000);
      state = await soakState();
      assert.equal(
        state.closed,
        null,
        `the soak socket closed mid-soak: ${JSON.stringify(state.closed)}`,
      );
      assert.equal(state.errors, 0, 'the soak socket reported an error event');
      assert.equal(state.readyState, 1, 'the soak socket left OPEN mid-soak');
      assert.equal(
        socketClosures.length,
        0,
        `a page WebSocket closed mid-soak: ${socketClosures.join(', ')}`,
      );
      if (!reloadObserved && existsSync(options.reloadMarker)) {
        reloadObserved = true;
        sequenceAtReload = state.pings.length === 0 ? 0 : state.pings[state.pings.length - 1];
        process.stdout.write(`public-soak: reload observed at ping sequence ${sequenceAtReload}\n`);
      }
    }

    state = await soakState();
    assert.equal(state.closed, null, 'the soak socket must still be open at the end');
    assert.equal(state.readyState, 1, 'the soak socket must end in OPEN');
    assert.equal(state.errors, 0, 'the soak socket must end with zero error events');
    assert.equal(state.undecodable, 0, 'every server frame must decode');
    assert.equal(socketClosures.length, 0, 'no page WebSocket may close during the soak');
    assert.deepEqual(consoleErrors, [], `page errors during the soak: ${consoleErrors.join('; ')}`);
    assertPingContinuity(state.pings, options.expectedPings);
    assert.equal(
      state.acks,
      state.pings.length,
      'every application PING must be acknowledged',
    );
    assert.ok(reloadObserved, 'the harness never signalled its caddy reload');
    assertReloadCrossing(state.pings, sequenceAtReload);

    process.stdout.write(
      `P2_H9_SOAK_BROWSER_PASS duration=${options.durationSeconds} pings=${state.pings.length} acks=${state.acks} reload_at=${sequenceAtReload}\n`,
    );
  } catch (error) {
    failure = error;
  } finally {
    // Tell the harness the soak window is over BEFORE the held socket goes
    // away, so its 30-second sampler cannot record the teardown as a
    // zero-connection sample.
    if (options.doneMarker) {
      try {
        writeFileSync(options.doneMarker, `${new Date().toISOString()}\n`);
      } catch {}
    }
    await browser.close();
  }
  if (failure) {
    throw failure;
  }
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    selfTest();
    return;
  }
  if (options.analyzeMemory) {
    const verdict = analyzeMemorySamples(readSampleFile(options.analyzeMemory));
    process.stdout.write(
      `public-soak memory: PASS samples=${verdict.samples} growth_bytes=${Math.round(verdict.growth)}\n`,
    );
    return;
  }
  if (options.analyzeConnections) {
    const verdict = analyzeConnectionSamples(readSampleFile(options.analyzeConnections));
    process.stdout.write(`public-soak connections: PASS samples=${verdict.samples}\n`);
    return;
  }
  await runSoak(options);
}

// fileURLToPath, not URL.pathname: the latter keeps percent-encoding and
// breaks on paths with spaces, silently turning every mode into an exit-0
// no-op -- a judgment tool must never "pass" by failing to run.
const invokedDirectly =
  process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));
if (invokedDirectly) {
  main().catch((error) => {
    process.stderr.write(`${error?.stack ?? error}\n`);
    process.exit(1);
  });
}
