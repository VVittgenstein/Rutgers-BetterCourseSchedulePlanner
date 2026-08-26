# Rutgers Better Course Schedule Planner for Windows

RBCSP is a portable, local application for searching Rutgers courses, applying
course and Section filters, viewing current Section status, and watching open
Sections. This archive is for 64-bit x86-64 Windows; it is not an ARM64 build.
The exact release version is recorded in `VERSION`.

## What you need

- A 64-bit x86-64 Windows computer.
- A current default web browser.
- A folder in which your Windows account can create and update files.
- Outbound HTTPS access to the official Rutgers Catalog and open-Sections
  services when you want current course or availability data.

RBCSP does not require administrator access, Node.js, npm, Python, Git, a
separate SQLite installation, OpenSSL, or a background service. Do not extract
it into a read-only or administrator-owned location such as `Program Files`.

## How the local application works

Run `Start-RBCSP.bat`, or run `RBCSP.exe` directly. One local RBCSP process
starts on a random loopback port and opens the application in your default
browser. The browser talks only to that local process. Opening another copy
while RBCSP is already running reuses the existing local instance instead of
creating another database or Rutgers polling process.

Keep the console window open while using RBCSP. Closing one tab has no effect
while another RBCSP page remains open. Closing the last page removes the
physical watches while retaining the stored desired-watch choices, then starts
a visible 60-second exit countdown. A page that returns during the countdown
cancels the exit and re-materializes every still-watchable choice; otherwise
the process shuts down in order. You can also press Ctrl+C in the console for
an immediate manual exit. Wait for the process to stop before moving, backing
up, or replacing files.

## Your data

The release archive contains no database, seed, preloaded course records, open-
Section records, Saved views, history, or other user data. On first start,
RBCSP creates its schema and its single primary database at:

```text
<the folder containing RBCSP.exe>\data\rbcsp.sqlite
```

Rutgers course and open-Section data appear only after this copy of RBCSP has
successfully fetched them. Settings, selected Sections, desired-watch choices,
Saved views, and local history are stored in the same package-relative
database. SQLite may create temporary sidecar files while the application is
running; do not copy the database until RBCSP has stopped.

The `data` folder deliberately travels with the extracted application folder.
Moving the complete folder after RBCSP has stopped moves both the program and
its local data. Deleting the complete folder also deletes that data.

## Network and privacy

RBCSP binds only to the local loopback interface and gives each run a new local
session value. Do not share the private loopback URL shown by a running copy.

The single local process makes the Rutgers Catalog and open-Sections requests;
browser tabs do not contact Rutgers independently. RBCSP does not send Saved
views, local history, settings, desired-watch choices, or the local database to
Rutgers. A currently open page may, with your permission, use a page-level
browser notification when it cannot be heard; this is not cloud sync, email,
server push, a service-worker notification, or a public-server service. Course
and availability responses fetched from Rutgers are stored locally so that
RBCSP can search them and report their freshness.

## Offline behavior

RBCSP can open without an Internet connection. If the database already
contains Rutgers data, the application may continue to show it with its actual
freshness or stale status. Search results and open-Section state cannot become
more current until Rutgers is reachable again, and watches cannot observe new
changes while offline. A first start while offline still creates the empty
local database, but it cannot show courses until a later successful refresh.

## Reset is not uninstall

The Settings page exposes three deliberately different reset scopes:

- Reset current filters clears only the current filter definition.
- Delete Saved views clears only the Saved view library.
- Reset all local user data stops watches and clears desired-watch choices,
  local settings, filters, Saved views, selected Sections, and local history.
  It retains Catalog and open-Section operational data and does not delete the
  database file.

To remove everything, first exit RBCSP and then delete the complete extracted
folder. Back up `data` before doing so if you may want the local state later.

## Package integrity and licensing

`SHA256SUMS` records SHA-256 hashes for the other package files.
`MANIFEST.json` records the package contents, and `BUILD-PROVENANCE.json`
records the build inputs. Dependency information and notices are in
`SBOM.cdx.json` and `THIRD-PARTY-NOTICES.txt`.

RBCSP is provided under the ISC License. See `LICENSE`.

For installation, backup, upgrade, recovery, and troubleshooting steps, read
`QUICKSTART.md`.
