# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT


def run(iterations, seed):
    x = seed
    for i in range(iterations):
        if x % 2 == 0:
            x = x // 2 + i
        else:
            x = (3 * x + 1) % 65521
    return x
