SELECT id, 'LIMIT 3 random()' AS note
FROM orders
ORDER BY id
LIMIT 5
