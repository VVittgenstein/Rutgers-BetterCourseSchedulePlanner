# P7.4-005-R2 - independent runner replay

GitHub-hosted run `29408621103` passed all four gates:

- Windows package and standard-user acceptance;
- Linux package, Caddy and browser acceptance;
- targeted fake-upstream product flow;
- equivalent current-signature malware scan of both candidates.

This no-product-change workflow marker triggers a second independent push run.
It must rebuild the exact same Windows and Linux archive bytes and repeat every
gate. It is a new run, not a rerun of the first.
