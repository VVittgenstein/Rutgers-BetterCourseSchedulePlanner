# BetterCourseSchedulePlanner
A modern course filtering and sniping tool for Rutgers University.

## Database migrations
The local SQLite schema lives in `data/schema.sql` and is versioned through SQL files inside `data/migrations`. Use the CLI below to initialize or upgrade a database:

```bash
npm run db:migrate                     # uses data/local.db by default
npm run db:migrate -- --db /tmp/csp.db # custom path
```

Flags supported by `scripts/migrate_db.ts`:

- `--migrations <dir>`: override migrations directory (default `data/migrations`)
- `--log-file <path>`: where to write append-only logs (default `data/migrations.log`)
- `--dry-run`: print the migrations that would run without touching the database
- `--verbose`: print status for already-applied migrations
