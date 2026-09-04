# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT


def run(iterations, seed):
    x = seed
    for i in range(iterations):
        x = ((x << 1) ^ (x >> 15) ^ i) & 65535
    return x
