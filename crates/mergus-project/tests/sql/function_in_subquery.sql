SELECT * FROM stg_orders WHERE id IN (SELECT NEXTVAL('seq'))
