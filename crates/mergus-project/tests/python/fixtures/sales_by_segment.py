import polars as pl


def joined(fact_sales, erp__rates, seed__countries):
    return """
        SELECT f.*, r.rate FROM fact_sales f JOIN erp__rates r USING (date)
    """


def _rate(frame):
    def inner(column):
        return pl.col(column)

    return frame.with_columns(inner("rate"))


def sales_by_segment(in_usd):
    return "SELECT segment, SUM(amount_usd) AS total_usd FROM in_usd GROUP BY ALL"


def in_usd(joined):
    return _rate(joined).with_columns((pl.col("amount") * pl.col("rate")).alias("amount_usd"))


class Helper:
    def method(self):
        return "SELECT 1"
