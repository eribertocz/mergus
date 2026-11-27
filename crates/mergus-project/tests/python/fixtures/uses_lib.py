import os

import helpers
from polars import col


def uses_lib(stg_orders):
    return helpers.in_usd(stg_orders).with_columns(col(os.sep))
