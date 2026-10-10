-- The tick-box grouping from 005 is replaced by a category you pick for each dish (Beef,
-- Chicken, Soups...), editable in Settings. 005 never shipped, so nothing is lost here.
ALTER TABLE item_ingredient DROP COLUMN group_by;

CREATE TABLE item_category (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE COLLATE NOCASE
);
INSERT INTO item_category (name) VALUES
  ('Beef'), ('Chicken'), ('Lamb'), ('Pork'), ('Seafood'), ('Vegetarian');

-- A dish (or a template's chip) has at most one category; deleting a category just unsets it.
ALTER TABLE item     ADD COLUMN category_id INTEGER REFERENCES item_category(id) ON DELETE SET NULL;
ALTER TABLE template ADD COLUMN category_id INTEGER REFERENCES item_category(id) ON DELETE SET NULL;
