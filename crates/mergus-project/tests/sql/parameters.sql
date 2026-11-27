SELECT *
FROM stg_orders
WHERE order_date = $run_date AND amount > $min_amount
