SELECT *
FROM stg_orders
WHERE amount > $min_amount
