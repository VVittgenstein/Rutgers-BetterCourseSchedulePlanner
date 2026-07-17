# P7.5-001-R2 completion record

The replacement Windows and Linux candidates passed the two-package verifier
at hashes `eabdc3b5f4a705d8c22e6941831f55e0bb5b5c2a1c33e648e545f86007cab577`
and `9bebd35808497e40ae36cb459c681ffb2ffe29c3b824988e460745d29f03605d`.
They contain no database or preloaded Rutgers data.

Windows standard-user prerequisites, zero-residue checks, environment-variable
checks, and a newly established Chrome control session passed. The candidate
was not started and made zero real Rutgers requests. The dynamic `2N+5` budget,
480-second window, serial 15-minute lease, and Chrome-primary evidence rule are
unchanged.

Gate: `P7_5_WINDOWS_REAL_WORLD_ELIGIBLE_R2`.
PostPush marker: `P7_5_001_R2_PASS_POST_PUSH`.
Next task: `P7.5-002-R3`.
