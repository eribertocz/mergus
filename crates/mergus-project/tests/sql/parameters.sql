SELECT *, $run_date AS run_date
FROM raw.erp__orders
WHERE updated_at >= $period_start AND updated_at < $period_end
