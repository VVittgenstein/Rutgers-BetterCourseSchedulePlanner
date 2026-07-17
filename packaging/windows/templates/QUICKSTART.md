# RBCSP Windows quick start

## 1. Extract the complete archive

Create a dedicated folder that your Windows account can write to, for example:

```text
C:\Users\<you>\Applications\RBCSP
```

Extract the complete archive into that folder. Do not run RBCSP inside the ZIP
viewer, do not copy `RBCSP.exe` by itself, and do not use `Program Files` or
another read-only or administrator-owned folder. The extracted folder should
contain the twelve release files, including `RBCSP.exe`, `Start-RBCSP.bat`,
`VERSION`, `MANIFEST.json`, and `SHA256SUMS`. It should not contain a `data`
folder before the first run.

If your distribution channel publishes a SHA-256 hash for the ZIP, compare it
before opening the archive. After extraction, `SHA256SUMS` can be compared with
the output of the built-in PowerShell `Get-FileHash -Algorithm SHA256` command.
Do not run a package whose expected and actual hashes differ.

## 2. Start RBCSP

Double-click `Start-RBCSP.bat`. You can also run `RBCSP.exe` directly. Keep the
console window open. RBCSP creates `data\rbcsp.sqlite` beside the program and
opens a private loopback page in your default browser.

The first database is empty: the archive does not ship Rutgers course or open-
Section data. With Internet access, RBCSP obtains current data from the
official Rutgers services. Multiple browser tabs share the one local process
and do not each poll Rutgers.

If a copy is already running, starting RBCSP again should open that existing
local instance. It does not create a second database.

## 3. Exit safely

Closing the browser tab alone does not stop RBCSP. Return to the RBCSP console,
press Ctrl+C, and wait for the process to stop. Exit before copying the
database, upgrading the program, moving the folder, restoring a backup, or
deleting the folder.

## Offline use

RBCSP can start while offline. Existing course data remains local and is shown
with its real freshness or stale state, but searches and open-Section status
cannot become more current and watches cannot observe new changes until
Rutgers is reachable. On a first offline start, the local database is created
without course data; results appear only after a later successful refresh.

## Back up local data

1. Exit RBCSP completely.
2. Copy the entire `data` folder to a backup location outside the RBCSP folder.
3. Label the backup with the value from `VERSION` and the backup date.
4. Keep the backup private: it can contain settings, selected Sections, Saved
   views, history, and fetched Rutgers data.

Do not back up only a live `-wal` or `-shm` sidecar, and do not copy the
database while RBCSP is running. The complete stopped `data` folder is the
backup unit.

## Upgrade without losing data

1. Exit the current RBCSP process.
2. Back up the complete `data` folder.
3. Keep a copy of the current release archive for rollback.
4. Extract the new archive into a separate temporary folder and verify its
   hashes and `VERSION`.
5. Copy the new release's twelve root files into the existing RBCSP folder,
   replacing the old root files but leaving the existing `data` folder intact.
   Do not delete the whole existing folder first.
6. Start the new `RBCSP.exe` and verify search, settings, Saved views, history,
   freshness, and watch behavior. Active watches intentionally do not resume
   automatically after a restart.

Do not extract a new archive over a running process. Do not maintain two active
copies that point at independently copied versions of the same database.

## Recover or roll back

If an upgrade fails:

1. Exit RBCSP completely.
2. Preserve the failed folder for diagnosis, but do not continue writing to
   its database.
3. Restore the previous release's twelve root files.
4. Replace `data` with the complete backup made before the upgrade. Keep the
   failed `data` folder under a different name until recovery is confirmed.
5. Start the restored version and verify local state.

Always pair an older program with its matching pre-upgrade database backup.
Do not assume that a database opened by a newer version can be safely opened by
an older version, and do not attempt rollback by deleting individual tables.

## Reset or uninstall

Use the Settings page when you want to reset only filters, only Saved views, or
all local user data. The full local-user reset retains Catalog and open-Section
operational data and retains `data\rbcsp.sqlite`.

To uninstall everything:

1. Exit RBCSP completely.
2. Back up `data` if you may want it later.
3. Delete the complete extracted RBCSP folder.

RBCSP does not install a Windows service or require an uninstaller. Deleting
the folder deletes both the program and its package-relative local data.

## Troubleshooting

- **The data-directory message appears:** move the complete extracted folder
  to a location owned and writable by your Windows account.
- **The browser does not open:** confirm that Windows has a working default
  browser, then start RBCSP again.
- **No courses appear:** check the network connection and the freshness/status
  information in RBCSP. A first offline database contains no courses.
- **Open status or watches do not update:** Rutgers must be reachable; stale or
  unavailable status is not a current open-Section observation.
- **A second launch reports an instance problem:** close any stale RBCSP
  process, wait a few seconds, and try once more.
- **The launcher reports a nonzero exit code:** read the message printed above
  it, verify that all twelve release files remain together, and confirm the
  folder is writable.

Do not disable Windows security controls merely to run an unverified package.
Verify the package hash and follow your organization's security policy.
