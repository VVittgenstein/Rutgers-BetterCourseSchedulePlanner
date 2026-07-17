#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

type CLIOptions = {
  dbPath: string;
  migrationsDir: string;
  logFile: string;
  dryRun: boolean;
  verbose: boolean;
};

type Migration = {
  version: string;
  name: string;
  filePath: string;
  checksum: string;
  contents: string;
};

const MIGRATION_TABLE = 'schema_migrations';
const MIGRATION_PATTERN = /^(\d+)_([\w-]+)\.sql$/;

function parseArgs(argv: string[]): CLIOptions {
  const defaults = {
    dbPath: path.resolve('data', 'local.db'),
    migrationsDir: path.resolve('data', 'migrations'),
    logFile: path.resolve('data', 'migrations.log'),
    dryRun: false,
    verbose: false,
  };

  const args = { ...defaults };

  const nextValue = (label: string, index: number) => {
    const value = argv[index];
    if (!value) {
      throw new Error(`Missing value for ${label}`);
    }
    return value;
  };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === '--db' || token === '--database') {
      args.dbPath = path.resolve(nextValue(token, i + 1));
      i += 1;
    } else if (token === '--migrations') {
      args.migrationsDir = path.resolve(nextValue(token, i + 1));
      i += 1;
    } else if (token === '--log-file') {
      args.logFile = path.resolve(nextValue(token, i + 1));
      i += 1;
    } else if (token === '--dry-run') {
      args.dryRun = true;
    } else if (token === '--verbose') {
      args.verbose = true;
    } else {
      throw new Error(`Unknown argument: ${token}`);
    }
  }

  return args;
}

function loadMigrations(dir: string): Migration[] {
  if (!fs.existsSync(dir)) {
    throw new Error(`Migrations directory not found: ${dir}`);
  }

  const files = fs
    .readdirSync(dir)
    .filter((file) => file.endsWith('.sql'))
    .sort();

  return files.map((file) => {
    const match = file.match(MIGRATION_PATTERN);
    if (!match) {
      throw new Error(`Migration file must follow <version>_<name>.sql: ${file}`);
    }

    const [, version, name] = match;
    const filePath = path.join(dir, file);
    const contents = fs.readFileSync(filePath, 'utf-8');
    const checksum = crypto.createHash('sha256').update(contents).digest('hex');

    return { version, name, filePath, checksum, contents };
  });
}

function ensureMigrationsTable(db: Database.Database) {
  db.exec(`
    CREATE TABLE IF NOT EXISTS ${MIGRATION_TABLE} (
      version TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      checksum TEXT NOT NULL,
      applied_at TEXT NOT NULL DEFAULT (datetime('now'))
    )
  `);
}

function getAppliedMigrations(
  db: Database.Database,
): Map<string, { checksum: string; name: string }> {
  const rows = db
    .prepare(`SELECT version, name, checksum FROM ${MIGRATION_TABLE}`)
    .all() as Array<{ version: string; name: string; checksum: string }>;

  return new Map(rows.map((row) => [row.version, { name: row.name, checksum: row.checksum }]));
}

function logToFile(logFile: string, message: string) {
  fs.mkdirSync(path.dirname(logFile), { recursive: true });
  fs.appendFileSync(logFile, `${new Date().toISOString()} ${message}\n`);
}

function main() {
  const opts = parseArgs(process.argv.slice(2));

  fs.mkdirSync(path.dirname(opts.dbPath), { recursive: true });
  const db = new Database(opts.dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');

  try {
    ensureMigrationsTable(db);

    const migrations = loadMigrations(opts.migrationsDir);
    if (migrations.length === 0) {
      console.log(`No migrations found in ${opts.migrationsDir}`);
      return;
    }

    const applied = getAppliedMigrations(db);
    let appliedCount = 0;

    for (const migration of migrations) {
      const existing = applied.get(migration.version);
      if (existing) {
        if (existing.checksum !== migration.checksum) {
          throw new Error(
            `Checksum mismatch for migration ${migration.version} (${migration.name}).` +
              ' Was the file modified after being applied?',
          );
        }
        if (opts.verbose) {
          console.log(`Skipping ${migration.version}_${migration.name} (already applied)`);
        }
        continue;
      }

      if (opts.dryRun) {
        const message = `[dry-run] would apply migration ${migration.version}_${migration.name}`;
        console.log(message);
        logToFile(opts.logFile, `${path.basename(opts.dbPath)} ${message}`);
        continue;
      }

      const apply = db.transaction(() => {
        db.exec(migration.contents);
        db
          .prepare(
            `INSERT INTO ${MIGRATION_TABLE} (version, name, checksum) VALUES (?, ?, ?)`,
          )
          .run(migration.version, migration.name, migration.checksum);
      });

      apply();
      appliedCount += 1;

      const message = `applied migration ${migration.version}_${migration.name}`;
      console.log(message);
      logToFile(opts.logFile, `${path.basename(opts.dbPath)} ${message}`);
    }

    if (appliedCount === 0 && !opts.dryRun) {
      console.log('Database already up to date.');
    }
  } finally {
    db.close();
  }
}

main();
