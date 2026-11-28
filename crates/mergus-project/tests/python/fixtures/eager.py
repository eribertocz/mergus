import polars as pl
import polars.selectors as cs


def _rows(frame):
    return frame.to_dicts()


def collected(stg_orders):
    return stg_orders.collect().lazy()


def eager(collected):
    frame = pl.DataFrame({"a": [1]})
    cs.DataFrame()
    collect(frame)
    collected.fetch(5)
    return collected.select(pl.col("a")).sort("a")


def wide(collected):
    return collected.collect_async()


def converted(collected):
    collected.to_pandas()
    return collected.to_arrow()
