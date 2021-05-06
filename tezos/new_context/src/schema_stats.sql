
PRAGMA foreign_keys = ON;
PRAGMA synchronous = OFF;

CREATE TABLE IF NOT EXISTS blocks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  hash TEXT UNIQUE,
  actions_count INTEGER,
  checkout_time_irmin REAL,
  checkout_time_tezedge REAL,
  commit_time_irmin REAL,
  commit_time_tezedge REAL
);

CREATE TABLE IF NOT EXISTS operations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  hash TEXT UNIQUE
);

CREATE TABLE IF NOT EXISTS contexts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  hash TEXT UNIQUE
);

CREATE TABLE IF NOT EXISTS actions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT,
  key_root TEXT,
  key TEXT,
  irmin_time REAL,
  tezedge_time REAL,
  block_id INTEGER DEFAULT NULL,
  operation_id INTEGER DEFAULT NULL,
  context_id INTEGER DEFAULT NULL,
  FOREIGN KEY(block_id) REFERENCES blocks(id),
  FOREIGN KEY(operation_id) REFERENCES operations(id),
  FOREIGN KEY(context_id) REFERENCES contexts(id)
);
