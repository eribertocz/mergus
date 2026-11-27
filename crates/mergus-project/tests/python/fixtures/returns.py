import polars as pl

TABLE = "orders"


def literal():
    return "SELECT " "1 AS one"


def frame(literal):
    return literal.filter(pl.col("one") > 0)


def fstring():
    return f"SELECT * FROM {TABLE}"


def concat():
    return "SELECT * FROM " + TABLE


def percent():
    return "SELECT * FROM %s" % TABLE


def bare():
    query = "SELECT 1"
    return query


def formatted():
    return "SELECT * FROM {}".format(TABLE)


def nothing():
    return None


def no_return():
    pass


def mixed(literal):
    if TABLE:
        return "SELECT 1"
    return literal.head(1)


def returns(frame):
    return "SELECT * FROM frame"
