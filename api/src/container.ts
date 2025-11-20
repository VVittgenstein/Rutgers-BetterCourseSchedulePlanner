import Database from 'better-sqlite3';

import type { AppConfig } from './config.js';

export interface AppContainer {
  config: AppConfig;
  getDb: () => Database.Database;
  close: () => void;
}

export function buildContainer({ config }: { config: AppConfig }): AppContainer {
  let db: Database.Database | null = null;

  const getDb = () => {
    if (!db) {
      db = new Database(config.sqliteFile, { fileMustExist: true });
      db.pragma('journal_mode = WAL');
      db.pragma('foreign_keys = ON');
    }
    return db;
  };

  const close = () => {
    if (db) {
      db.close();
      db = null;
    }
  };

  return { config, getDb, close };
}
