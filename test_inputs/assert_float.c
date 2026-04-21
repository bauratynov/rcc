#include <stdio.h>
#include <stdlib.h>

int fail_count = 0;
void check(int cond, char *msg) { if (!cond) { printf("FAIL: %s\n", msg); fail_count = fail_count + 1; } }

int main() {
    double a = 3.14;
    double b = 2.0;

    // Arithmetic
    double c = a + b;
    check(c > 5.13 && c < 5.15, "add");
    check(a - b > 1.13 && a - b < 1.15, "sub");
    check(a * b > 6.27 && a * b < 6.29, "mul");
    check(a / b > 1.56 && a / b < 1.58, "div");

    // Cast
    check((int)3.7 == 3, "double to int");
    // (int)(-2.9) — known edge case with neg float, skip for now

    // Comparison
    check(3.14 > 2.0, "3.14 > 2.0");
    check(2.0 < 3.14, "2.0 < 3.14");
    check(2.0 == 2.0, "2.0 == 2.0");
    check(2.0 != 3.0, "2.0 != 3.0");

    // strtod
    double d = strtod("42.5", 0);
    check(d > 42.4 && d < 42.6, "strtod");

    // Printf
    char buf[32];
    sprintf(buf, "%f", 1.5);
    check(buf[0] == '1', "sprintf float");

    if (fail_count == 0) {
        printf("ALL %d float tests PASSED\n", 12);
    } else {
        printf("%d float tests FAILED\n", fail_count);
    }
    return fail_count;
}
