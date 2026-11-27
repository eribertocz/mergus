WITH stg_orders AS (SELECT * FROM raw.erp__orders)
SELECT * FROM stg_orders JOIN raw.seed__countries USING (country)
