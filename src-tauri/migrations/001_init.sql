CREATE TABLE ingredient_category (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE ingredient (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  category_id INTEGER NOT NULL REFERENCES ingredient_category(id),
  unit        TEXT NOT NULL CHECK (unit IN ('g','ml','each')),
  archived    INTEGER NOT NULL DEFAULT 0
);

-- Templates are for meals only.
CREATE TABLE template (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL
);

CREATE TABLE item (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  type         TEXT NOT NULL CHECK (type IN ('meal','meat','side')),
  colour       TEXT,
  notes        TEXT,
  template_id  INTEGER REFERENCES template(id),
  archived     INTEGER NOT NULL DEFAULT 0
);

-- For type = 'meat', quantity is PER SERVE.
CREATE TABLE item_ingredient (
  id            INTEGER PRIMARY KEY,
  item_id       INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  ingredient_id INTEGER NOT NULL REFERENCES ingredient(id),
  quantity      REAL NOT NULL,
  position      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE template_line (
  id            INTEGER PRIMARY KEY,
  template_id   INTEGER NOT NULL REFERENCES template(id) ON DELETE CASCADE,
  label         TEXT NOT NULL,
  ingredient_id INTEGER REFERENCES ingredient(id),
  category_id   INTEGER REFERENCES ingredient_category(id),
  quantity      REAL,
  unit_hint     TEXT,
  position      INTEGER NOT NULL DEFAULT 0
);

-- Dinner only: one row per item on a day.
CREATE TABLE plan_entry (
  id        INTEGER PRIMARY KEY,
  date      TEXT NOT NULL,
  item_id   INTEGER NOT NULL REFERENCES item(id),
  serves    INTEGER,
  note      TEXT,
  position  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_plan_entry_date ON plan_entry(date);
CREATE INDEX idx_plan_entry_item ON plan_entry(item_id);
CREATE INDEX idx_item_ingredient_item ON item_ingredient(item_id);

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
