# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT


def run(iterations, seed):
    exponent = iterations
    base = seed % 65521
    result = 1
    while exponent != 0:
        if exponent & 1:
            result = result * base % 65521
        base = base * base % 65521
        exponent >>= 1
    return result
