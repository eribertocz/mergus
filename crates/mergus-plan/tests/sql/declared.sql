SELECT id, ANY_VALUE(note) AS note, 'random() now()' AS text
FROM orders
GROUP BY id
ORDER BY id
LIMIT 5
