# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT

"""Type-dispatched readers for external data, without implicit coercion.

Only input boundaries need these readers. Once decoded, values travel through
the support code as typed objects; domain constraints stay with their callers.
"""

import math
from functools import singledispatch


@singledispatch
def require_number(value: object) -> float:
    raise ValueError("finite numeric value required")


@require_number.register(int)
@require_number.register(float)
def _number(value: float) -> float:
    result = float(value)
    if not math.isfinite(result):
        raise ValueError("finite numeric value required")
    return result


@require_number.register
def _not_boolean(value: bool) -> float:
    raise ValueError("finite numeric value required")


@singledispatch
def require_text(value: object) -> str:
    raise ValueError("text value required")


@require_text.register
def _text(value: str) -> str:
    return value


@singledispatch
def require_integer(value: object) -> int:
    raise ValueError("integer inputs and results required")


@require_integer.register
def _integer(value: int) -> int:
    return value


@require_integer.register
def _boolean(value: bool) -> int:
    # bool inherits from int, but must not silently become a workload number.
    raise ValueError("integer inputs and results required")


@singledispatch
def require_table(value: object) -> dict[str, object]:
    raise ValueError("table with text keys required")


@require_table.register(dict)
def _table(value: dict[object, object]) -> dict[str, object]:
    return {require_text(key): item for key, item in value.items()}


@singledispatch
def require_list(value: object) -> list[object]:
    raise ValueError("list required")


@require_list.register(list)
def _list(value: list[object]) -> list[object]:
    return value
