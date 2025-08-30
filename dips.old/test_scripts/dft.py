import random
import math

threshold = 1e-15


def e_2_pi_i(value, k, n, N):
    a = value * math.cos(2 * math.pi * k * n / N)
    if math.fabs(a) < threshold:
        a = 0
    b = value * -math.sin(2 * math.pi * k * n / N)
    if math.fabs(b) < threshold:
        b = 0
    # print(a, b)
    return (a, b)


def complex_add(val1, val2):
    return (
        val1[0] + val2[0],
        val1[1] + val2[1]
    )


def mag(complex):
    return math.sqrt(complex[0] ** 2 + complex[1] ** 2)


size = 4

# vals = [random.random() * 100 for i in range(100)]
# vals = [i % 5 - 2 for i in range(size)]
vals = [0, 1, 0, -1]
output = []

for j in range(len(vals)):
    sum = (0, 0)
    print()
    for i in range(size):
        v = e_2_pi_i(vals[j], j, i, size)
        print(v)
        sum = complex_add(sum, v)
    # output.append(mag(sum))
    # output.append(int(mag(sum)))
    output.append(sum)

print(output)
# print(vals)
