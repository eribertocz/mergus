# A Python model whose imports would fail: Mergus reads it without importing it.
import polars as pl
from .helpers import clean as tidy
import mergus_module_that_does_not_exist

raise SystemExit("this file was executed")


def joined(fact_sales, exchange_rates):
    return """
        SELECT f.*, r.rate FROM fact_sales f JOIN exchange_rates r USING (date)
    """


def in_usd(joined, /, *, scale=1, **options):
    def nested():
        return 0
    if scale is None:
        return
    return joined.sort("date").first().with_columns(
        (pl.col("amount") * pl.col("rate")).alias("amount_usd")
    ).unique(subset=["date"], **options)


async def latest(in_usd, *rest):
    return f"SELECT * FROM {in_usd}"
