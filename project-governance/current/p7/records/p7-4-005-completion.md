# P7.4-005 completion record

- Parent: `91744cbfbc691834f6668ca10b0b27389ce38ba1`
- Validated workflow commit: `91744cbfbc691834f6668ca10b0b27389ce38ba1`
- Frozen product source: `7d8297404d033e79b514333748b7072ebd3a0099`
- Successful clean runs: `29391985651`, `29392739883`

The two independent runs produced byte-identical candidate payloads:

- Windows: `eb85374bbf97215124b4f2b64be4c51c96bc2af0502fc79b5230024709590610`
  (`5,534,122` bytes, `11` files);
- Linux: `77160882304fbe4d17a070a1cce16471cd618a1cd5cee18c9a2f6e9e8e920d07`
  (`6,366,047` bytes, `20` files).

Windows standard-user/browser/Defender, Ubuntu Caddy/browser/install/restore/
upgrade/rollback, targeted fake-upstream, ClamAV, and both downloaded-pair joint
audits passed. Shared SBOM/frontend counts were `169`/`10`; absolute and DOS 8.3
Windows user-path hits were `0`.

P7.4-004 remains immutable historical evidence for its exact bytes. Those bytes
and the later `69719b9b…cada4` Windows intermediate were superseded; only the two
full hashes above are canonical.

- Real-world E2E completed: `false`.
- P7 completed: `false`.
- GitHub Release created/authorized/eligible: `false`.
- Vultr, staging, DNS, Cloudflare, certificate, and production mutation: `false`.
- Production deployment authorized: `false`.

Gate: `P7_5_ELIGIBLE`.
Commit boundary: `P7_4_FINAL_DEDICATED_CANDIDATE_GATE_COMMIT`.
Required PostPush marker: `P7_4_005_PASS_POST_PUSH`.
Next task after PostPush: `P7.5-001`.
