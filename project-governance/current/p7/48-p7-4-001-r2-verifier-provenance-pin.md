# P7.4-001-R2 - verifier provenance pin correction

The builders and workflow already pinned frozen source `7d5debe`, but the two
package verifiers still expected the preceding source identity. Both verifiers
now require commit `7d5debef005277e4d8f2ed2b9fb2f72c495e62f1` and epoch
`1784109539`. No product, package layout, dependency or candidate hash changed.

The superseded discovery run is not acceptance evidence. Next task: corrected
official-runner hash discovery.
