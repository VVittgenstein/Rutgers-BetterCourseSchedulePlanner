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
// The server's acknowledgement counter is a whole-process aggregate, so
// --analyze-acks first proves the window belonged to this browser alone --
// nothing connected before it armed, exactly one admission granted, exactly
// one connection at every sample -- and only then reads the delta as this
// socket's. A second socket could otherwise supply the acknowledgements the
// target connection never had accepted.
//
// Modes:
//   (soak)                node public-soak-browser.mjs --base-url URL \
//                           --playwright-root PATH --armed-marker FILE \
//                           --reload-marker FILE --ack-report FILE \
//                           [--done-marker FILE] \
//                           [--duration-seconds 600] [--expected-pings 50]
//   --analyze-memory F    judge `epoch bytes` MemoryCurrent samples
//   --analyze-connections F  judge `epoch count` connection-gauge samples
//     (both require --window-start/--window-end epochs [--interval-seconds
//      30] and refuse thin, holed, late, or truncated coverage)
//   --analyze-acks        prove the judged window held ONE connection
//                           (--connections-before 0, and exactly one
//                           admission across --admissions-baseline and
//                           --admissions-final), then hold the SERVER's
//                           accepted-ACK counter delta (--ack-baseline /
//                           --ack-final, read inside one service invocation)
//                           against what the browser half reported sending
//                           (--ack-report)
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
/** Scheduling slack allowed around the 30-second sampling cadence. */
export const SAMPLE_JITTER_SECONDS = 15;
/**
 * How many acknowledgements the browser may have sent without the server
 * having accepted them by the time the counter is read.
 *
 * The boundary is crossed in BOTH directions. The page counts an ACK the
 * instant it hands the frame to the socket, so the server may not have taken
 * the last one yet; and the page keeps answering pings until its browser
 * closes, after the report was written, so the server may have taken one the
 * report does not mention. One frame either way is the whole gap at a
 * ten-second ping cadence -- large enough for the tail, too small to hide a
 * server that stopped accepting.
 */
export const ACK_DELIVERY_TOLERANCE = 1;

/**
 * Parses `epoch value` sample lines. A failed read is recorded by the
 * sampler as a SAMPLE_READ_FAILURE value; it must refuse here -- evidence
 * with holes punched by failing commands is not evidence.
 */
export function parseTimestampedSamples(text) {
  const lines = text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '');
  return lines.map((line) => {
    const fields = line.split(/\s+/);
    assert.equal(fields.length, 2, `sample lines are "epoch value", got: ${line}`);
    const epoch = Number.parseInt(fields[0], 10);
    const value = Number.parseInt(fields[1], 10);
    assert.ok(Number.isSafeInteger(epoch) && epoch > 0, `unparsable sample epoch: ${line}`);
    assert.ok(Number.isSafeInteger(value), `unparsable sample value (a failed read?): ${line}`);
    return { epoch, value };
  });
}

/**
 * The R1 coverage judgment: the samples must blanket the WHOLE soak
 * window at the promised cadence. Too few samples, a late start, a hole
 * in the middle, or a missing tail each fail -- one sample per soak, or
 * six for a 600-second window, proves nothing about the minutes between.
 */
export function assertWindowCoverage(samples, { startEpoch, endEpoch, intervalSeconds }) {
  assert.ok(
    Number.isSafeInteger(startEpoch) && Number.isSafeInteger(endEpoch) && endEpoch > startEpoch,
    'the coverage window must be a real epoch range',
  );
  assert.ok(
    Number.isSafeInteger(intervalSeconds) && intervalSeconds > 0,
    'the sampling interval must be positive',
  );
  const windowSeconds = endEpoch - startEpoch;
  // The count floor allows the full jitter on EVERY cycle (a loaded
  // sampler drifts a few seconds per iteration); the gap rule below is
  // what catches holes, and the first/last rules catch clipped ends.
  const required = Math.max(
    2,
    Math.floor(windowSeconds / (intervalSeconds + SAMPLE_JITTER_SECONDS)),
  );
  assert.ok(
    samples.length >= required,
    `only ${samples.length} samples for a ${windowSeconds}s window; the ${intervalSeconds}s cadence requires at least ${required}`,
  );
  const slack = intervalSeconds + SAMPLE_JITTER_SECONDS;
  assert.ok(
    samples[0].epoch <= startEpoch + slack,
    `the first sample (${samples[0].epoch}) misses the start of the window (${startEpoch})`,
  );
  const last = samples[samples.length - 1];
  assert.ok(
    last.epoch >= endEpoch - slack,
    `the last sample (${last.epoch}) misses the end of the window (${endEpoch})`,
  );
  for (let index = 1; index < samples.length; index += 1) {
    const gap = samples[index].epoch - samples[index - 1].epoch;
    assert.ok(gap >= 0, 'sample epochs must not go backwards');
    assert.ok(
      gap <= slack,
      `a ${gap}s hole between samples ${index - 1} and ${index} breaks the ${intervalSeconds}s cadence`,
    );
  }
  return { samples: samples.length, windowSeconds };
}

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

/**
 * Parses one reading of a monotonic server counter.
 *
 * The shell writes a literal failure marker when it cannot read the metric,
 * and an absent metric reads as an empty string. Both must refuse here: a
 * gate that treats an unreadable counter as zero, or as "probably fine",
 * proves nothing at all.
 */
export function parseCounterReading(text, label) {
  assert.ok(
    typeof text === 'string' && /^[0-9]+$/.test(text.trim()),
    `${label} is not a counter reading (a failed read?): ${JSON.stringify(text)}`,
  );
  const value = Number.parseInt(text.trim(), 10);
  assert.ok(Number.isSafeInteger(value) && value >= 0, `${label} is out of range: ${text}`);
  return value;
}

/**
 * The R2 acceptance judgment: the SERVER must say it accepted the
 * acknowledgements this soak's browser sent.
 *
 * Before R2 the evidence was "the page called send()" plus "the journal shows
 * no rejection" -- a service that silently dropped every valid ACK while
 * still refreshing its heartbeat from arbitrary inbound text passed both.
 * Here the delta of the server's own accepted-ACK counter, read inside one
 * service invocation, has to match what the browser actually sent.
 */
export function assertAcceptedAckEvidence({ baseline, finalReading, browserAcks, expectedMinimum }) {
  const before = parseCounterReading(baseline, 'the accepted-ACK baseline');
  const after = parseCounterReading(finalReading, 'the accepted-ACK final reading');
  assert.ok(
    Number.isSafeInteger(browserAcks) && browserAcks > 0,
    `the browser must report the acknowledgements it sent, got ${browserAcks}`,
  );
  assert.ok(
    Number.isSafeInteger(expectedMinimum) && expectedMinimum > 0,
    'the expected acknowledgement floor must be positive',
  );
  // A counter that went backwards is a restarted process or a reading from
  // somewhere else; either way the two readings cannot be subtracted.
  assert.ok(
    after >= before,
    `the accepted-ACK counter went backwards (${before} -> ${after}); the readings are not comparable`,
  );
  const accepted = after - before;
  assert.ok(
    accepted >= expectedMinimum,
    `the server accepted ${accepted} heartbeat ACK(s) during the soak; the gate requires at least ${expectedMinimum}`,
  );
  // The bound is only meaningful because assertSoleAdmission has shown the
  // window held one connection. On its own this arm is satisfied by any
  // mixture of sockets summing to the expected number -- which is exactly
  // how a second socket could once cover for a target whose ACKs the server
  // ignored.
  assert.ok(
    accepted <= browserAcks + ACK_DELIVERY_TOLERANCE,
    `the server accepted ${accepted} ACK(s) but the browser sent ${browserAcks}; the counter is not counting acknowledgements`,
  );
  assert.ok(
    accepted >= browserAcks - ACK_DELIVERY_TOLERANCE,
    `the browser sent ${browserAcks} ACK(s) but the server accepted ${accepted}; the acknowledgements did not land`,
  );
  return { accepted, browserAcks };
}

/** Reads the browser half's own report of what it sent. */
export function parseAckReport(text) {
  const report = JSON.parse(text);
  assert.ok(report && typeof report === 'object', 'the ACK report must be an object');
  assert.ok(
    Number.isSafeInteger(report.pings) && report.pings > 0,
    `the ACK report must carry the observed ping count, got ${report.pings}`,
  );
  assert.ok(
    Number.isSafeInteger(report.acks) && report.acks > 0,
    `the ACK report must carry the sent acknowledgement count, got ${report.acks}`,
  );
  assert.equal(
    report.acks,
    report.pings,
    'the browser half must acknowledge every ping it observed',
  );
  return report;
}

/**
 * Judges the connection-gauge samples: EXACTLY one public watch connection,
 * for every sample in the judged window.
 *
 * ">= 1" was the wrong question. The accepted-ACK counter is a whole-process
 * aggregate, so a second socket answering pings of its own feeds it exactly
 * as the soak socket does -- and a window that tolerated a 2 would let that
 * second socket supply the acknowledgements the target connection never had
 * accepted. One means one.
 */
export function analyzeConnectionSamples(samples) {
  assert.ok(samples.length > 0, 'no connection samples were recorded');
  for (const sample of samples) {
    assert.ok(
      Number.isSafeInteger(sample) && sample === 1,
      `every connection sample must be exactly 1 -- the soak holds one socket and nothing else may be connected -- got ${sample}`,
    );
  }
  return { samples: samples.length };
}

/**
 * The soak's ACK evidence is a whole-process aggregate, so the window has to
 * be shown to contain exactly one connection: none before the browser
 * arrived, and exactly one admission granted across the whole run.
 *
 * The gauge alone cannot say that -- a socket that dropped and was replaced
 * reads as 1 at every sample. The admissions counter is monotonic, so its
 * delta counts replacements too: one connection that stayed is a delta of
 * one, and a reconnect, a second page, or a probe is not.
 */
export function assertSoleAdmission({ connectionsBefore, admissionsBaseline, admissionsFinal }) {
  const before = parseCounterReading(connectionsBefore, 'the pre-soak connection gauge');
  assert.equal(
    before,
    0,
    `${before} public watch connection(s) were already open before the soak armed; the window would not be this browser's alone`,
  );
  const baseline = parseCounterReading(admissionsBaseline, 'the admissions baseline');
  const finalReading = parseCounterReading(admissionsFinal, 'the admissions final reading');
  assert.ok(
    finalReading >= baseline,
    `the admissions counter went backwards (${baseline} -> ${finalReading}); the readings are not comparable`,
  );
  const admissions = finalReading - baseline;
  assert.equal(
    admissions,
    1,
    `${admissions} public watch admission(s) were granted during the soak; this evidence is only the soak socket's if there was exactly one`,
  );
  return { admissions };
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

function readCoveredSamples(path, options) {
  assert.ok(
    Number.isSafeInteger(options.windowStart) && Number.isSafeInteger(options.windowEnd),
    'the analyze modes need --window-start and --window-end (fail closed without them)',
  );
  const samples = parseTimestampedSamples(readFileSync(path, 'utf8'));
  assertWindowCoverage(samples, {
    startEpoch: options.windowStart,
    endEpoch: options.windowEnd,
    intervalSeconds: options.intervalSeconds,
  });
  return samples.map((sample) => sample.value);
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

  // Connections: exactly one, every sample. Zero is a lost socket and two is
  // a second connection whose acknowledgements would land in the same
  // aggregate counter the soak reads.
  analyzeConnectionSamples([1, 1, 1, 1]);
  assert.throws(() => analyzeConnectionSamples([1, 0, 1]), /exactly 1/);
  assert.throws(
    () => analyzeConnectionSamples([1, 1, 2, 1]),
    /exactly 1/,
    'a second concurrent connection must fail the window',
  );
  assert.throws(() => analyzeConnectionSamples([]), /no connection samples/);

  // Sole admission: nothing connected before the browser, exactly one
  // admission granted across the run.
  const soleCase = (overrides) =>
    assertSoleAdmission({
      connectionsBefore: '0',
      admissionsBaseline: '7',
      admissionsFinal: '8',
      ...overrides,
    });
  assert.deepEqual(soleCase({}), { admissions: 1 });
  assert.throws(
    () => soleCase({ connectionsBefore: '1' }),
    /already open before the soak armed/,
    'a connection that predates the browser makes the window ambiguous',
  );
  assert.throws(
    () => soleCase({ admissionsFinal: '7' }),
    /0 public watch admission/,
    'the browser must have been admitted at all',
  );
  assert.throws(
    () => soleCase({ admissionsFinal: '9' }),
    /2 public watch admission/,
    'a reconnect or a second page is a second admission',
  );
  assert.throws(
    () => soleCase({ admissionsBaseline: '9', admissionsFinal: '8' }),
    /went backwards/,
    'a restarted counter cannot be subtracted',
  );
  // A poisoned capacity lock reports u32::MAX, which is a well-formed
  // number and would sail through any range check. These are equality rules
  // for exactly that reason.
  assert.throws(() => soleCase({ connectionsBefore: '4294967295' }), /already open before/);
  assert.throws(() => soleCase({ admissionsFinal: '4294967295' }), /public watch admission/);
  for (const unreadable of ['COUNTER_READ_FAILURE', '', null, undefined]) {
    assert.throws(() => soleCase({ connectionsBefore: unreadable }), /not a counter reading/);
    assert.throws(() => soleCase({ admissionsBaseline: unreadable }), /not a counter reading/);
    assert.throws(() => soleCase({ admissionsFinal: unreadable }), /not a counter reading/);
  }

  // THE R3 DISCRIMINATOR. The soak socket had every acknowledgement ignored;
  // a second socket, connected for part of the window, had exactly as many
  // accepted. The ACK arithmetic alone cannot tell the difference -- it sees
  // the delta it expected -- and before R3 that was the whole judgment.
  const secondSocket = {
    connectionsBefore: '0',
    admissionsBaseline: '4',
    admissionsFinal: '6',
    ackBaseline: '10',
    ackFinal: '65',
    browserAcks: 55,
    connectionSamples: [1, 1, 2, 2, 1],
  };
  assert.deepEqual(
    assertAcceptedAckEvidence({
      baseline: secondSocket.ackBaseline,
      finalReading: secondSocket.ackFinal,
      browserAcks: secondSocket.browserAcks,
      expectedMinimum: 50,
    }),
    { accepted: 55, browserAcks: 55 },
    'the aggregate ACK judgment is satisfied by the impostor, which is why it cannot stand alone',
  );
  assert.throws(
    () =>
      assertSoleAdmission({
        connectionsBefore: secondSocket.connectionsBefore,
        admissionsBaseline: secondSocket.admissionsBaseline,
        admissionsFinal: secondSocket.admissionsFinal,
      }),
    /2 public watch admission/,
    'the second socket was admitted, and admissions do not lie about that',
  );
  assert.throws(
    () => analyzeConnectionSamples(secondSocket.connectionSamples),
    /exactly 1/,
    'and it was visible in the window while it was connected',
  );

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

  // Window coverage (R1): the samples must blanket the whole soak.
  const cadence = (start, count, interval) =>
    Array.from({ length: count }, (_, index) => ({ epoch: start + index * interval, value: 1 }));
  const window = { startEpoch: 1_000, endEpoch: 1_600, intervalSeconds: 30 };
  assertWindowCoverage(cadence(1_005, 20, 30), window);
  // Real-world drift: samples sliding a few seconds late still pass.
  assertWindowCoverage(cadence(1_010, 19, 31), window);
  assert.throws(
    () => assertWindowCoverage(cadence(1_005, 6, 30), window),
    /requires at least/,
    'six samples cannot vouch for a 600-second window',
  );
  assert.throws(
    () => assertWindowCoverage([{ epoch: 1_005, value: 1 }], window),
    /requires at least/,
    'one sample proves nothing',
  );
  assert.throws(
    () => {
      // Enough samples to satisfy the count, but two missing in the middle
      // leave a 90-second hole the cadence check must catch.
      const holed = cadence(1_005, 21, 30).filter((_, index) => index !== 9 && index !== 10);
      assertWindowCoverage(holed, window);
    },
    /hole between samples/,
    'a missing middle must fail',
  );
  assert.throws(
    () => assertWindowCoverage(cadence(1_005, 14, 30), window),
    /requires at least|misses the end/,
    'a missing tail must fail',
  );
  assert.throws(
    () => assertWindowCoverage(cadence(1_100, 19, 30), window),
    /misses the start/,
    'a late-starting sampler must fail',
  );
  assert.throws(
    () =>
      assertWindowCoverage(
        [{ epoch: 1_035, value: 1 }, { epoch: 1_005, value: 1 }, ...cadence(1_065, 19, 30)],
        window,
      ),
    /not go backwards/,
    'unsorted sample epochs must fail',
  );

  // Timestamped parsing: a failed read is a refusal, not a thin spot.
  assert.deepEqual(parseTimestampedSamples('1000 419430400\n1030 419430401\n'), [
    { epoch: 1000, value: 419430400 },
    { epoch: 1030, value: 419430401 },
  ]);
  assert.throws(
    () => parseTimestampedSamples('1000 419430400\n1030 SAMPLE_READ_FAILURE\n'),
    /failed read/,
    'a sampler failure line must refuse',
  );
  assert.throws(() => parseTimestampedSamples('419430400\n'), /epoch value/);

  // Accepted-ACK evidence (R2): the SERVER's count is the evidence, and the
  // browser's own tally is only what it is held against.
  const ackCase = (overrides) =>
    assertAcceptedAckEvidence({
      baseline: '10',
      finalReading: '65',
      browserAcks: 55,
      expectedMinimum: 50,
      ...overrides,
    });
  assert.deepEqual(ackCase({}), { accepted: 55, browserAcks: 55 });
  // One acknowledgement in flight when the counter is read is tolerated, in
  // either direction: the page keeps answering pings until its browser closes,
  // which happens after it wrote the report.
  assert.deepEqual(ackCase({ finalReading: '64' }), { accepted: 54, browserAcks: 55 });
  assert.deepEqual(ackCase({ finalReading: '66' }), { accepted: 56, browserAcks: 55 });
  // THE discriminator: the page sent 55 ACKs and the server accepted none.
  // Before R2 this soak passed -- the page had "sent" them and the journal
  // held no rejection.
  assert.throws(
    () => ackCase({ finalReading: '10' }),
    /requires at least/,
    'a server that silently ignores every valid ACK must fail the soak',
  );
  assert.throws(
    () => ackCase({ finalReading: '40', expectedMinimum: 20 }),
    /did not land/,
    'accepting far fewer ACKs than were sent must fail',
  );
  assert.throws(
    () => ackCase({ baseline: '100', finalReading: '40' }),
    /went backwards/,
    'a restarted (or unrelated) counter cannot be subtracted',
  );
  assert.throws(
    () => ackCase({ finalReading: '200' }),
    /not counting acknowledgements/,
    'a counter that outruns what the browser sent is not counting ACKs',
  );
  for (const unreadable of ['SAMPLE_READ_FAILURE', '', '  ', 'nan', null, undefined]) {
    assert.throws(
      () => ackCase({ baseline: unreadable }),
      /not a counter reading/,
      `an unreadable baseline (${JSON.stringify(unreadable)}) must refuse`,
    );
    assert.throws(
      () => ackCase({ finalReading: unreadable }),
      /not a counter reading/,
      `an unreadable final reading (${JSON.stringify(unreadable)}) must refuse`,
    );
  }
  assert.throws(
    () => ackCase({ browserAcks: 0 }),
    /acknowledgements it sent/,
    'a browser half that reports nothing cannot be correlated',
  );

  // The browser half's report must be complete and self-consistent.
  assert.deepEqual(parseAckReport('{"pings":55,"acks":55,"sequenceAtReload":27}'), {
    pings: 55,
    acks: 55,
    sequenceAtReload: 27,
  });
  assert.throws(() => parseAckReport('{"pings":55}'), /sent acknowledgement count/);
  assert.throws(() => parseAckReport('{"pings":55,"acks":54}'), /every ping it observed/);
  assert.throws(() => parseAckReport('{"pings":"55","acks":55}'), /observed ping count/);

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
    ackReport: null,
    analyzeAcks: false,
    ackBaseline: null,
    ackFinal: null,
    connectionsBefore: null,
    admissionsBaseline: null,
    admissionsFinal: null,
    windowStart: null,
    windowEnd: null,
    intervalSeconds: 30,
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
      case '--ack-report':
        options.ackReport = next();
        break;
      case '--analyze-acks':
        options.analyzeAcks = true;
        break;
      // Deliberately kept as raw text: the shell writes a failure marker
      // when it cannot read the counter, and the judgment must see that
      // rather than a silently coerced number.
      case '--ack-baseline':
        options.ackBaseline = next();
        break;
      case '--ack-final':
        options.ackFinal = next();
        break;
      case '--connections-before':
        options.connectionsBefore = next();
        break;
      case '--admissions-baseline':
        options.admissionsBaseline = next();
        break;
      case '--admissions-final':
        options.admissionsFinal = next();
        break;
      case '--window-start':
        options.windowStart = Number.parseInt(next(), 10);
        break;
      case '--window-end':
        options.windowEnd = Number.parseInt(next(), 10);
        break;
      case '--interval-seconds':
        options.intervalSeconds = Number.parseInt(next(), 10);
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
  // Without the report there is nothing to correlate the server's accepted
  // count against, so the soak could not prove acceptance even if it held.
  assert.ok(options.ackReport, '--ack-report is required');
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

    // What this half actually sent, for the shell to hold against the
    // server's own accepted-ACK counter.
    writeFileSync(
      options.ackReport,
      `${JSON.stringify({
        pings: state.pings.length,
        acks: state.acks,
        sequenceAtReload,
      })}\n`,
    );

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
    const verdict = analyzeMemorySamples(readCoveredSamples(options.analyzeMemory, options));
    process.stdout.write(
      `public-soak memory: PASS samples=${verdict.samples} growth_bytes=${Math.round(verdict.growth)}\n`,
    );
    return;
  }
  if (options.analyzeConnections) {
    const verdict = analyzeConnectionSamples(
      readCoveredSamples(options.analyzeConnections, options),
    );
    process.stdout.write(`public-soak connections: PASS samples=${verdict.samples}\n`);
    return;
  }
  if (options.analyzeAcks) {
    assert.ok(options.ackReport, '--analyze-acks needs the browser half\'s --ack-report');
    const report = parseAckReport(readFileSync(options.ackReport, 'utf8'));
    // Exclusivity first: an aggregate counter's delta says nothing about
    // THIS socket until the window is known to have held only it.
    const sole = assertSoleAdmission({
      connectionsBefore: options.connectionsBefore,
      admissionsBaseline: options.admissionsBaseline,
      admissionsFinal: options.admissionsFinal,
    });
    const verdict = assertAcceptedAckEvidence({
      baseline: options.ackBaseline,
      finalReading: options.ackFinal,
      browserAcks: report.acks,
      expectedMinimum: options.expectedPings,
    });
    process.stdout.write(
      `public-soak acks: PASS server_accepted=${verdict.accepted} browser_sent=${verdict.browserAcks} admissions=${sole.admissions}\n`,
    );
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
