async def async_result():
    return "SELECT 1"


def reader(async_result):
    return "SELECT 2"
