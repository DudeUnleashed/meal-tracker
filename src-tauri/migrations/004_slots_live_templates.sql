-- Rebuild `item` so `type` also allows 'sweet' (Sweet/Savory) and 'blank' (the takeaway token).
-- A CHECK can't be altered in place. Foreign keys are switched off around migrations (see db.rs)
-- so dropping the old table doesn't touch the rows that point at it.
CREATE TABLE item_new (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  type         TEXT NOT NULL CHECK (type IN ('meal','meat','side','sweet','blank')),
  colour       TEXT,
  notes        TEXT,
  template_id  INTEGER REFERENCES template(id),
  archived     INTEGER NOT NULL DEFAULT 0,
  sample       INTEGER NOT NULL DEFAULT 0
);
INSERT INTO item_new (id, name, type, colour, notes, template_id, archived, sample)
  SELECT id, name, type, colour, notes, template_id, archived, sample FROM item;
DROP TABLE item;
ALTER TABLE item_new RENAME TO item;

-- Meal slots. Everything planned so far was a dinner.
ALTER TABLE plan_entry ADD COLUMN slot TEXT NOT NULL DEFAULT 'dinner'
  CHECK (slot IN ('breakfast','lunch','dinner','snack'));
-- Title of a Blank (takeaway etc.) entry. Unlike `note`, it is the content, so copying keeps it.
ALTER TABLE plan_entry ADD COLUMN label TEXT;

-- Live templates: each template has a chip (a linked item) that is dragged onto the plan.
-- `item_id` is filled in by the app on startup (db.rs); the old item.template_id stays as
-- "this meal was started from that template" and is unrelated.
ALTER TABLE template ADD COLUMN item_type TEXT NOT NULL DEFAULT 'meal'
  CHECK (item_type IN ('meal','side','sweet'));
ALTER TABLE template ADD COLUMN colour TEXT;
ALTER TABLE template ADD COLUMN item_id INTEGER REFERENCES item(id);
CREATE UNIQUE INDEX idx_template_item ON template(item_id) WHERE item_id IS NOT NULL;

-- A slot with multi = 1 lets you tick several ingredients on a night (e.g. steamed veg).
ALTER TABLE template_line ADD COLUMN multi INTEGER NOT NULL DEFAULT 0;

-- Candidate ingredients for a slot, each with its own default amount in its own unit.
CREATE TABLE template_option (
  id            INTEGER PRIMARY KEY,
  line_id       INTEGER NOT NULL REFERENCES template_line(id) ON DELETE CASCADE,
  ingredient_id INTEGER NOT NULL REFERENCES ingredient(id),
  quantity      REAL,
  position      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_template_option_line ON template_option(line_id);

-- What one night's chip picked for the template's slots. The amount is copied in when picked,
-- so editing the template later doesn't change nights that are already planned.
CREATE TABLE plan_entry_pick (
  id            INTEGER PRIMARY KEY,
  entry_id      INTEGER NOT NULL REFERENCES plan_entry(id) ON DELETE CASCADE,
  line_id       INTEGER REFERENCES template_line(id) ON DELETE SET NULL,
  ingredient_id INTEGER NOT NULL REFERENCES ingredient(id),
  quantity      REAL NOT NULL,
  position      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_plan_entry_pick_entry ON plan_entry_pick(entry_id);

-- New shopping categories go at the end so any order you've set is left alone.
INSERT OR IGNORE INTO ingredient_category (name, sort_order)
  VALUES ('Bakery', (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM ingredient_category));
INSERT OR IGNORE INTO ingredient_category (name, sort_order)
  VALUES ('Frozen', (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM ingredient_category));

-- The demo frozen foods, unless they've already been moved somewhere else.
UPDATE ingredient SET category_id = (SELECT id FROM ingredient_category WHERE name = 'Frozen')
  WHERE sample = 1
    AND ((name = 'Frozen peas'  AND category_id = (SELECT id FROM ingredient_category WHERE name = 'Veg'))
      OR (name = 'Frozen chips' AND category_id = (SELECT id FROM ingredient_category WHERE name = 'Pantry')));
