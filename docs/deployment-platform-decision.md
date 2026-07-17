# B v1 Deployment Platform Decision and Mainline Handoff

Status: Accepted for planning; infrastructure provisioned  
Date: 2026-07-11  
Scope: Preflight 0B for the public web deployment (B)

This document replaces the earlier OCI-only draft. It records the complete
decision trail from the infrastructure side branch and gives the mainline a
single safe source for continuing product planning. It is not an NGAT handoff
and does not authorize application implementation or public launch.

## Executive Decision

- **B v1 will deploy to a new Vultr Cloud Compute instance in New Jersey
  (`ewr`).**
- The selected machine class is **Shared CPU, AMD High Performance**, with
  1 vCPU, 1 GB RAM, 25 GB NVMe storage, and 2 TB monthly transfer at a base
  price of USD 6/month.
- The instance has been created with Ubuntu 24.04 LTS x64 and a dedicated BCSP
  SSH key. SSH key authentication has been tested successfully.
- **OCI Always Free is no longer the selected or blocking B v1 platform.** It
  remains only a possible future cost-reduction option if Oracle eventually
  approves the account.
- ChatGPT Sites, Cloudflare Workers/Pages, and Google Cloud Run are not the B v1
  real-time backend target.
- No domain has been purchased or assigned. Raw-IP validation is acceptable for
  infrastructure and early HTTP testing, but a domain and trusted HTTPS are
  required before a public launch.

## Product Runtime That Drove the Decision

B is not a static site. It must support the product behavior fixed in
`docs/public-web-target.md`:

- the same ordinary-user WebUI as A, usable on desktop and mobile;
- no email reminders and no mail configuration;
- a server-maintained course catalog, refreshed every 10 minutes for B;
- one centralized Rutgers openSections poller rather than one poller per user;
- active browser-session subscriptions, with at most 9 sections per session;
- live status delivery to browsers, with WebSocket selected as the preferred
  B v1 transport;
- browser audio for every fresh Open status message for a watched section, even
  if the section was already Open;
- no debounce, plus one-shot/continuous audio and volume controls;
- an aspirational near-1-second path from a fresh Rutgers result to browser
  alert, subject to upstream and load-test evidence.

This runtime needs an always-on process that can retain in-memory connection
state, run a continuous poller, serve HTTP and WebSocket traffic, and use local
SQLite storage. A conventional Linux VM is the simplest fit.

## Decision Trail

### 1. OCI Always Free

OCI was initially selected because an always-on Linux VM matched the workload
and the Always Free tier could remove monthly hosting cost.

The user accepted card verification and attempted registration with legitimate
identity and billing information:

- US Chase card;
- New Jersey billing/home address;
- current physical presence in Shenzhen;
- earlier access through a US VPS, followed by a direct mainland connection.

Oracle/CyberSource rejected registration. A support request accurately
explaining the cross-border situation and requesting manual review was
submitted, but Oracle did not reply in time to support the project schedule.

Mainline conclusion:

- do not block B planning or implementation on Oracle;
- do not create duplicate Oracle accounts or manipulate identity/location data;
- retain the support case only as a passive future option;
- a later successful OCI account may justify migration, but it is not part of
  B v1 acceptance.

### 2. ChatGPT Sites and Cloudflare Serverless Products

Sites was considered after it became available in Codex/ChatGPT. It may be
useful for ordinary interactive websites, but its public beta documentation did
not provide a stable numeric capacity commitment for this workload, nor a clear
production contract for all of the following together:

- long-lived WebSocket/SSE connections;
- one continuously resident poller;
- shared in-memory connection coordination;
- a one-second active polling target;
- predictable public availability after usage limits are reached.

Cloudflare Pages/Workers can host UI and request/response logic, but moving the
real-time design there would introduce lifecycle, state-coordination, and quota
questions without removing the need for a durable polling backend. Browser
polling once per second was rejected because 50 continuously active users could
produce up to 4.32 million client requests per day.

Mainline conclusion:

- do not use Sites or a pure Workers design as the B v1 backend;
- Cloudflare remains a good later choice for DNS, TLS edge/proxying, and related
  perimeter services after a domain is selected;
- the React frontend should remain portable rather than depend on a Sites-only
  runtime.

### 3. Google Cloud Run

Cloud Run was not selected because its request-driven lifecycle, possible cold
starts, background-process constraints, and usage-based billing are less
natural for a small always-on poller plus live-connection service than one
predictably priced VM.

### 4. Vultr

Vultr was selected after the account was available and actual EWR inventory was
checked. The USD 6 AMD High Performance plan was chosen over the USD 5 regular
plan because one additional dollar provides a newer AMD EPYC class, NVMe
storage, and 2 TB rather than 1 TB of transfer.

The USD 10 regular 2 GB plan was available but not selected. With a Rust native
backend, SQLite, Caddy, and a target peak of about 50 active users, 1 GB should
be a reasonable starting point. This is a capacity hypothesis, not a promise;
P7 must still include memory, WebSocket, catalog-refresh, and poller load tests.

The server must not compile the complete Rust/React project in production.
Cross-platform CI or a build machine should produce release artifacts, and the
VPS should run the audited deployment package.

## Provisioned Baseline

Public-safe facts:

- provider: Vultr;
- region: New Jersey / New York (NJ), `ewr`;
- label: `Rutgers BCSP Public EWR 01`;
- hostname: `bcsp-public-ewr-01`;
- plan: `vhp-1c-1gb-amd`;
- CPU: 1 vCPU, AMD EPYC-Genoa class;
- memory: 1 GB;
- storage: 25 GB NVMe;
- transfer: 2 TB/month;
- base price: USD 6/month;
- OS: Ubuntu 24.04 LTS x64;
- public IPv4 and IPv6: assigned;
- cloud-init: complete;
- swap: approximately 2.5 GB;
- automatic backups: disabled;
- application: not deployed;
- domain/TLS: not configured.

Automatic Vultr backups would add 20 percent (USD 1.20/month). They were
disabled because B v1 has no persistent user subscriptions, the course catalog
can be rebuilt, and the application/deployment configuration should be
reproducible from audited artifacts. This decision must be revisited before B
stores irreplaceable server-side data.

Stopping a Vultr instance does not stop billing. The machine is now an ongoing
USD 6/month project cost until it is destroyed or resized.

## Verified Access and Local Records

The new machine uses a dedicated Ed25519 key; the unrelated
`vultr-ingress-sjc-01` instance and its access path were not reused or modified.

Safe mainline entry points:

- SSH alias: `ssh bcsp-public`;
- human-readable private operations record:
  `.secrets/infrastructure/bcsp-public-ewr-01.md`;
- machine-readable project-local inventory:
  `configs/bcsp-public.local.json`;
- SSH client config: `%USERPROFILE%/.ssh/config`.

`configs/bcsp-public.local.json` is covered by the existing
`configs/*.local.json` ignore rule. It contains infrastructure identifiers and
connection metadata but no private key material. The dedicated private/public
key pair is stored under `.secrets/ssh/`. The entire project-local `.secrets/`
tree is covered by an anchored `/.secrets/` ignore rule and must remain local.

The following must never enter Git, NGAT artifacts, release packages, GitHub
Releases, logs, screenshots, or public documentation:

- SSH private key material;
- root password;
- provider API tokens or billing data;
- future Cloudflare/domain credentials;
- unredacted local inventory intended to remain private.

Project-local storage means "inside this checkout for operational ownership,"
not "part of the Git repository." The `.secrets/` directory must never be
force-added, staged, committed, archived into a release, copied into an NGAT
worktree, or included in a deployment package.

The root password was deliberately not retrieved because SSH key access is
verified and sufficient.

## Current Security State

The server is reachable and SSH key authentication works. Current baseline:

- UFW is active;
- inbound traffic is denied by default;
- only TCP 22 is allowed on IPv4 and IPv6;
- only SSH is publicly listening;
- no Vultr firewall group is attached;
- SSH password authentication is still enabled;
- direct root SSH login is still permitted;
- `systemctl` reports `degraded` only because the VM-inapplicable
  `fwupd.service` and `fwupd-refresh.service` failed.

Before public deployment, the approved deployment procedure must:

1. create and verify a non-root administrative/deployment user;
2. preserve a tested recovery path before changing SSH policy;
3. disable SSH password authentication;
4. restrict direct root SSH login;
5. retain UFW default-deny behavior;
6. open ports 80 and 443 only when Caddy/application deployment is ready;
7. optionally attach a Vultr firewall group as a second perimeter layer;
8. configure log retention appropriate for a 25 GB disk;
9. verify automatic security updates and time synchronization;
10. rerun an external port and TLS audit after deployment.

Security hardening should be represented in the later B implementation/deploy
plan, not performed ad hoc without a rollback path.

## Domain and Cloudflare Decision

No domain purchase has been approved or completed.

Agreed boundary:

- raw IP is enough for SSH, health checks, and early HTTP validation;
- raw IP is not the intended public product address;
- a normal browser-trusted HTTPS deployment should use a domain;
- Caddy remains the preferred origin reverse proxy and certificate manager;
- Cloudflare is a likely DNS/proxy layer, not the application runtime.

Mainline must still decide:

1. the public product/domain name;
2. registrar and acceptable annual budget;
3. whether Cloudflare proxying is enabled immediately or DNS-only first;
4. the DNS cutover and rollback procedure;
5. whether the origin should accept traffic only through Cloudflare after the
   direct-origin launch is proven.

## Architecture Dependency

The capacity and deployment choice assume the accepted 0C direction documented
in `docs/shared-rust-architecture-decision.md`:

- one React/Vite/TypeScript WebUI;
- one shared Rust business core;
- two thin composition roots, Windows-local and Linux-public;
- Tokio + Axum + Tower + SQLx + SQLite;
- current open status and active subscriptions in memory;
- a centralized single-flight Rutgers poller;
- WebSocket delivery for active browser sessions;
- one native Linux process behind Caddy and managed by systemd;
- no required Docker, Redis, PostgreSQL, microservices, or Kubernetes for v1.

## Mainline Handoff State

Completed in this side branch:

- evaluated OCI registration and escalated the legitimate failure to support;
- evaluated Sites/serverless suitability for the real-time workload;
- selected Vultr as B v1 platform;
- selected and provisioned the EWR AMD High Performance plan;
- created a dedicated SSH key and local SSH aliases;
- verified provider/API identity, IPv4/IPv6, key fingerprints, and live SSH;
- persisted a private ignored inventory;
- recorded the shared Rust architecture direction;
- left the machine with UFW exposing SSH only and no application installed.

Still pending in the mainline:

- approve this file as the final 0B record;
- choose the domain/Cloudflare path before public launch;
- include baseline hardening in the eventual B deployment plan;
- run P1 and its review gate before P2-P6 planning;
- do not treat the existence of the VPS as approval to skip the agreed NGAT
  phases or to deploy an unaudited application;
- load-test the 1 GB target during P7 and define a resize trigger;
- produce and audit the B deployment package before using this server for the
  public product.

The 0B infrastructure-selection question is therefore closed: **Vultr EWR is
the B v1 deployment target, the machine exists and is reachable, and domain
selection remains a separate mainline decision.**
