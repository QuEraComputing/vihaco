# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT


def run(iterations, seed):
    mask = (1 << 64) - 1
    value = seed
    other = 305419896
    for _ in range(iterations):
        value ^= other
        value = ((value >> 13) | (value << 51)) & mask
        value = (((value << 7) & mask) & other) ^ seed
        other = ((other << 3) | (other >> 61)) & mask
    return value & 65535
