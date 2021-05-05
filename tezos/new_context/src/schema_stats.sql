
PRAGMA foreign_keys = ON;
PRAGMA synchronous = OFF;

CREATE TABLE IF NOT EXISTS blocks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  hash TEXT UNIQUE,
  tezedge_time_max REAL,
  tezedge_time_mean REAL,
  tezedge_time_total REAL,
  irmin_time_max REAL,
  irmin_time_mean REAL,
  irmin_time_total REAL
);

CREATE TABLE IF NOT EXISTS operations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  hash TEXT UNIQUE
);

CREATE TABLE IF NOT EXISTS contexts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  hash TEXT UNIQUE
);

CREATE TABLE IF NOT EXISTS block_details (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  action_name TEXT NOT NULL,
  irmin_time REAL NOT NULL,
  tezedge_time REAL NOT NULL,
  block_id INTEGER DEFAULT NULL,
  FOREIGN KEY(block_id) REFERENCES blocks(id)
);

CREATE TABLE IF NOT EXISTS actions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT,
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

CREATE TABLE IF NOT EXISTS global_stats (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  actions_count INTEGER,
  tezedge_checkouts_max REAL,
  tezedge_checkouts_mean REAL,
  tezedge_checkouts_total REAL,
  irmin_checkouts_max REAL,
  irmin_checkouts_mean REAL,
  irmin_checkouts_total REAL,
  tezedge_commits_max REAL,
  tezedge_commits_mean REAL,
  tezedge_commits_total REAL,
  irmin_commits_max REAL,
  irmin_commits_mean REAL,
  irmin_commits_total REAL
);

INSERT INTO global_stats (id) VALUES (0) ON CONFLICT DO NOTHING;
