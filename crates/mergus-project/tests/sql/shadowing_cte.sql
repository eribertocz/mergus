WITH stg_orders AS (SELECT 1 AS a), ghost AS (SELECT * FROM stg_orders)
SELECT * FROM ghost
