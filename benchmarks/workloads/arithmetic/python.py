# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT


def run(iterations, seed):
    x = seed
    for i in range(iterations):
        x = (17 * x + i) % 65521
    return x
