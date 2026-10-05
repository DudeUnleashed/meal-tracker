INSERT INTO ingredient_category (name, sort_order) VALUES
  ('Meat', 1), ('Seafood', 2), ('Veg', 3), ('Fruit', 4),
  ('Dairy', 5), ('Pantry', 6), ('Other', 7);

INSERT INTO ingredient (name, category_id, unit)
SELECT v.name, c.id, v.unit FROM (
  SELECT 'Beef mince' AS name, 'Meat' AS cat, 'g' AS unit UNION ALL
  SELECT 'Steak', 'Meat', 'g' UNION ALL
  SELECT 'Chicken thigh', 'Meat', 'g' UNION ALL
  SELECT 'Chicken breast', 'Meat', 'g' UNION ALL
  SELECT 'Pork mince', 'Meat', 'g' UNION ALL
  SELECT 'Bacon', 'Meat', 'g' UNION ALL
  SELECT 'Sausages', 'Meat', 'each' UNION ALL
  SELECT 'Chicken Kiev', 'Meat', 'each' UNION ALL
  SELECT 'Salmon fillet', 'Seafood', 'each' UNION ALL
  SELECT 'Prawns', 'Seafood', 'g' UNION ALL
  SELECT 'Carrot', 'Veg', 'each' UNION ALL
  SELECT 'Zucchini', 'Veg', 'each' UNION ALL
  SELECT 'Potato', 'Veg', 'g' UNION ALL
  SELECT 'Onion', 'Veg', 'each' UNION ALL
  SELECT 'Mushroom', 'Veg', 'g' UNION ALL
  SELECT 'Broccoli', 'Veg', 'each' UNION ALL
  SELECT 'Capsicum', 'Veg', 'each' UNION ALL
  SELECT 'Cabbage', 'Veg', 'g' UNION ALL
  SELECT 'Lettuce', 'Veg', 'each' UNION ALL
  SELECT 'Tomato', 'Veg', 'each' UNION ALL
  SELECT 'Frozen peas', 'Veg', 'g' UNION ALL
  SELECT 'Lemon', 'Fruit', 'each' UNION ALL
  SELECT 'Parmesan', 'Dairy', 'g' UNION ALL
  SELECT 'Cheese', 'Dairy', 'g' UNION ALL
  SELECT 'Cream', 'Dairy', 'ml' UNION ALL
  SELECT 'Butter', 'Dairy', 'g' UNION ALL
  SELECT 'Eggs', 'Dairy', 'each' UNION ALL
  SELECT 'Arborio rice', 'Pantry', 'g' UNION ALL
  SELECT 'Spaghetti', 'Pantry', 'g' UNION ALL
  SELECT 'Chow mein noodles', 'Pantry', 'g' UNION ALL
  SELECT 'Chicken stock', 'Pantry', 'ml' UNION ALL
  SELECT 'Pasta sauce', 'Pantry', 'ml' UNION ALL
  SELECT 'Soy sauce', 'Pantry', 'ml' UNION ALL
  SELECT 'Frozen chips', 'Pantry', 'g'
) v JOIN ingredient_category c ON c.name = v.cat;

INSERT INTO item (name, type, colour) VALUES
  ('Chow mein', 'meal', '#f59e0b'),
  ('Spag bol', 'meal', '#ef4444'),
  ('Steak', 'meat', '#b91c1c'),
  ('Sausages', 'meat', '#c2410c'),
  ('Chicken Kiev', 'meat', '#ca8a04'),
  ('Chips', 'side', '#eab308'),
  ('Garden salad', 'side', '#22c55e'),
  ('Steamed veg', 'side', '#16a34a');

-- Meat quantities are per serve.
INSERT INTO item_ingredient (item_id, ingredient_id, quantity, position)
SELECT i.id, g.id, v.qty, v.pos FROM (
  SELECT 'Chow mein' AS item, 'Beef mince' AS ing, 500.0 AS qty, 0 AS pos UNION ALL
  SELECT 'Chow mein', 'Chow mein noodles', 400, 1 UNION ALL
  SELECT 'Chow mein', 'Carrot', 2, 2 UNION ALL
  SELECT 'Chow mein', 'Cabbage', 300, 3 UNION ALL
  SELECT 'Chow mein', 'Onion', 1, 4 UNION ALL
  SELECT 'Chow mein', 'Soy sauce', 60, 5 UNION ALL
  SELECT 'Spag bol', 'Beef mince', 500, 0 UNION ALL
  SELECT 'Spag bol', 'Spaghetti', 400, 1 UNION ALL
  SELECT 'Spag bol', 'Pasta sauce', 500, 2 UNION ALL
  SELECT 'Spag bol', 'Onion', 1, 3 UNION ALL
  SELECT 'Spag bol', 'Carrot', 1, 4 UNION ALL
  SELECT 'Steak', 'Steak', 250, 0 UNION ALL
  SELECT 'Sausages', 'Sausages', 2, 0 UNION ALL
  SELECT 'Chicken Kiev', 'Chicken Kiev', 1, 0 UNION ALL
  SELECT 'Chips', 'Frozen chips', 600, 0 UNION ALL
  SELECT 'Garden salad', 'Lettuce', 1, 0 UNION ALL
  SELECT 'Garden salad', 'Tomato', 3, 1 UNION ALL
  SELECT 'Garden salad', 'Carrot', 1, 2 UNION ALL
  SELECT 'Steamed veg', 'Broccoli', 1, 0 UNION ALL
  SELECT 'Steamed veg', 'Carrot', 2, 1 UNION ALL
  SELECT 'Steamed veg', 'Zucchini', 1, 2
) v
JOIN item i ON i.name = v.item
JOIN ingredient g ON g.name = v.ing;

INSERT INTO template (name) VALUES ('Risotto'), ('Stir fry');

-- ingredient_id set = fixed line; NULL = slot to fill (filtered by category).
INSERT INTO template_line (template_id, label, ingredient_id, category_id, quantity, unit_hint, position)
SELECT t.id, v.label,
  (SELECT id FROM ingredient WHERE name = v.ing),
  (SELECT id FROM ingredient_category WHERE name = v.cat),
  v.qty, v.hint, v.pos
FROM (
  SELECT 'Risotto' AS tpl, 'Arborio rice' AS label, 'Arborio rice' AS ing, NULL AS cat, 300.0 AS qty, NULL AS hint, 0 AS pos UNION ALL
  SELECT 'Risotto', 'Chicken stock', 'Chicken stock', NULL, 1000, NULL, 1 UNION ALL
  SELECT 'Risotto', 'Parmesan', 'Parmesan', NULL, 50, NULL, 2 UNION ALL
  SELECT 'Risotto', 'Protein', NULL, 'Meat', 500, 'g', 3 UNION ALL
  SELECT 'Risotto', 'Veg 1', NULL, 'Veg', 200, 'g', 4 UNION ALL
  SELECT 'Stir fry', 'Soy sauce', 'Soy sauce', NULL, 60, NULL, 0 UNION ALL
  SELECT 'Stir fry', 'Protein', NULL, 'Meat', 500, 'g', 1 UNION ALL
  SELECT 'Stir fry', 'Veg 1', NULL, 'Veg', 2, 'each', 2 UNION ALL
  SELECT 'Stir fry', 'Veg 2', NULL, 'Veg', 2, 'each', 3
) v JOIN template t ON t.name = v.tpl;

INSERT INTO setting (key, value) VALUES
  ('default_meat_serves', '3'),
  ('week_start', '1');
