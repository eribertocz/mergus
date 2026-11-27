SELECT id, ANY_VALUE(note) OVER (PARTITION BY id) AS note
FROM orders
