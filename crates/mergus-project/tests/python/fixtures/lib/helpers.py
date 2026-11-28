import polars as pl
from rates import RATE
from . import ignored


def in_usd(frame):
    return frame.with_columns(pl.col("amount") * RATE)
