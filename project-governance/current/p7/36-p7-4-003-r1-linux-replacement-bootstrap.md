# P7.4-003-R1 - Linux replacement candidate bootstrap

The Linux builder and verifier are pinned to frozen source
`476565cbe8e19075214cdc1427c86cf2dcf4e966` and epoch `1784101290`. The GitHub
Actions Ubuntu and Windows jobs now build that same source, with the accepted
Windows replacement hash locked.

Because this host has no WSL, the deterministic Linux hash must be learned from
the first real Ubuntu build. The workflow deliberately retains the old Linux
hash check for this one discovery run. That run is not acceptance evidence and
is expected to stop at the hash comparison after the builder prints the new
SHA-256. No package is admitted by this task.

Next task: lock the observed Linux hash and run the final workflow twice.
