#include "test.h"

double add_double(double a, double b) { return a + b; }
double mul_double(double a, double b) { return a * b; }

int main() {
    // Basic double arithmetic
    double a = 3.14;
    double b = 2.0;
    assert_eq(1, a + b > 5.13 && a + b < 5.15, "add");
    assert_eq(1, a - b > 1.13 && a - b < 1.15, "sub");
    assert_eq(1, a * b > 6.27 && a * b < 6.29, "mul");
    assert_eq(1, a / b > 1.56 && a / b < 1.58, "div");

    // Double comparison
    assert_eq(1, 3.14 > 2.0, "3.14>2.0");
    assert_eq(0, 2.0 > 3.14, "2.0>3.14");
    assert_eq(1, 2.0 == 2.0, "2.0==2.0");
    assert_eq(1, 2.0 != 3.0, "2.0!=3.0");
    assert_eq(1, 2.0 < 3.0, "2.0<3.0");
    assert_eq(1, 2.0 <= 2.0, "2.0<=2.0");

    // Cast
    assert_eq(3, (int)3.7, "(int)3.7");
    assert_eq(42, (int)42.0, "(int)42.0");

    // Function with double
    assert_eq(1, add_double(1.5, 2.5) > 3.99 && add_double(1.5, 2.5) < 4.01, "add_double");
    assert_eq(1, mul_double(3.0, 4.0) > 11.99 && mul_double(3.0, 4.0) < 12.01, "mul_double");

    // Printf
    char buf[32];
    sprintf(buf, "%f", 1.5);
    assert_eq('1', buf[0], "sprintf float");

    test_summary();
    return 0;
}
