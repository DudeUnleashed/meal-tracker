-- A ticked ingredient on a recipe marks the dish as "grouped under" that ingredient in the
-- library lists (e.g. tick Beef mince on Beef rissoles). Nothing is ticked automatically.
ALTER TABLE item_ingredient ADD COLUMN group_by INTEGER NOT NULL DEFAULT 0;
