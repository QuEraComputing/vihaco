# SPDX-FileCopyrightText: 2026 The vihaco Authors
# SPDX-License-Identifier: MIT


def run(iterations, seed):
    frame = [seed + index for index in range(10)]
    for _ in range(iterations):
        previous = frame[9]
        for index in range(10):
            frame[index] = (frame[index] * 17 + previous) % 65521
            previous = frame[index]
    return sum(frame)
