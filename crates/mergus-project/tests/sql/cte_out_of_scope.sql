SELECT *
FROM (WITH inner_cte AS (SELECT 1 AS a) SELECT * FROM inner_cte) AS s, inner_cte
