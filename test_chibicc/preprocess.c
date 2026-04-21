#include "test.h"

#define TEN 10
#define ADD(a,b) ((a)+(b))
#define SQUARE(x) ((x)*(x))
#define MAX(a,b) ((a)>(b)?(a):(b))

#ifdef TEN
int has_ten = 1;
#else
int has_ten = 0;
#endif

#ifndef UNDEFINED_THING
int not_defined = 1;
#endif

#define A 1
#define B 2
#if A + B == 3
int ab_sum = 1;
#else
int ab_sum = 0;
#endif

int main() {
    assert_eq(10, TEN, "TEN");
    assert_eq(30, ADD(10, 20), "ADD");
    assert_eq(25, SQUARE(5), "SQUARE");
    assert_eq(20, MAX(10, 20), "MAX");
    assert_eq(1, has_ten, "ifdef");
    assert_eq(1, not_defined, "ifndef");
    assert_eq(1, ab_sum, "#if expr");

    // String concat
    char *s = "hello" " " "world";
    assert_eq(11, (int)strlen(s), "concat len");

    test_summary();
    return 0;
}
