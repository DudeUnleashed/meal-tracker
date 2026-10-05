-- Per-entry size for meals and sides ("1.5x for a bigger night"). Meat scales by serves instead.
ALTER TABLE plan_entry ADD COLUMN scale REAL NOT NULL DEFAULT 1.0;

-- Anything about a day itself rather than a dinner: a free-text note ("out for a birthday dinner")
-- and/or a Leftovers marker. A row exists only while it has a note or the marker.
CREATE TABLE day_info (
  date      TEXT PRIMARY KEY,
  note      TEXT,
  leftovers INTEGER NOT NULL DEFAULT 0
);

-- Marks the demo content shipped with the app so it can be removed in one go.
ALTER TABLE ingredient ADD COLUMN sample INTEGER NOT NULL DEFAULT 0;
ALTER TABLE item       ADD COLUMN sample INTEGER NOT NULL DEFAULT 0;
ALTER TABLE template   ADD COLUMN sample INTEGER NOT NULL DEFAULT 0;

UPDATE ingredient SET sample = 1 WHERE name IN (
  'Beef mince', 'Steak', 'Chicken thigh', 'Chicken breast', 'Pork mince', 'Bacon', 'Sausages',
  'Chicken Kiev', 'Salmon fillet', 'Prawns', 'Carrot', 'Zucchini', 'Potato', 'Onion', 'Mushroom',
  'Broccoli', 'Capsicum', 'Cabbage', 'Lettuce', 'Tomato', 'Frozen peas', 'Lemon', 'Parmesan',
  'Cheese', 'Cream', 'Butter', 'Eggs', 'Arborio rice', 'Spaghetti', 'Chow mein noodles',
  'Chicken stock', 'Pasta sauce', 'Soy sauce', 'Frozen chips'
);

UPDATE item SET sample = 1 WHERE name IN (
  'Chow mein', 'Spag bol', 'Steak', 'Sausages', 'Chicken Kiev', 'Chips', 'Garden salad', 'Steamed veg'
);

UPDATE template SET sample = 1 WHERE name IN ('Risotto', 'Stir fry');
