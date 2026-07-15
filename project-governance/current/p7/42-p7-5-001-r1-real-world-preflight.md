# P7.5-001-R1 - Real-world candidate and Chrome preflight

P7.4-005-R1 and its PostPush gate passed. The only eligible candidates are:

- Windows: `2c807e475779f9d3f7e8549921655dad87c4dff3b06286d05ac2a985fa2907d2`
- Linux: `67c4f5ac228dee7e4cc69378b12fbe294f3175f0bc453f5818e18704eba6cf04`

The Windows archive contains 11 files and no data directory. The Linux archive
contains 20 files and no database or preloaded course/Open data. Chrome control,
Secondary Logon, and the disposable-standard-user boundary are ready;
`BCSP_CI_NO_RUTGERS` is absent at process, user, and machine scope.

For each environment, discovery is limited to 2 attempts, Catalog to `N`, Open
to `N+3`, total Rutgers attempts to `2N+5`, and the live window to 480 seconds.
Local and Vultr decisive evidence must come from Chrome operating the real UI;
background checks are supplemental only. Public Chrome must use the Vultr
address through temporary `planner.test` hosts mapping and a CurrentUser-trusted
test root certificate, never localhost substitution.

Neither candidate was started during this task.

Gate: `P7_5_WINDOWS_REAL_WORLD_ELIGIBLE_R1`.
Next task: `P7.5-002-R1`.
