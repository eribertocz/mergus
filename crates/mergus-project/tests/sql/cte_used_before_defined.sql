WITH first_cte AS (SELECT * FROM second_cte), second_cte AS (SELECT 1 AS a)
SELECT * FROM first_cte
