#include "test.h"

int main() {
    // Basic arithmetic
    assert_eq(0, 0, "0");
    assert_eq(42, 42, "42");
    assert_eq(21, 5+20-4, "5+20-4");
    assert_eq(41, 12+34-5, "12+34-5");
    assert_eq(47, 5+6*7, "5+6*7");
    assert_eq(15, 5*(9-6), "5*(9-6)");
    assert_eq(4, (3+5)/2, "(3+5)/2");
    assert_eq(10, -10+20, "-10+20");
    assert_eq(10, - -10, "- -10");

    // Comparison
    assert_eq(0, 0==1, "0==1");
    assert_eq(1, 42==42, "42==42");
    assert_eq(1, 0!=1, "0!=1");
    assert_eq(0, 42!=42, "42!=42");
    assert_eq(1, 0<1, "0<1");
    assert_eq(0, 1<1, "1<1");
    assert_eq(0, 2<1, "2<1");
    assert_eq(1, 0<=1, "0<=1");
    assert_eq(1, 1<=1, "1<=1");
    assert_eq(0, 2<=1, "2<=1");
    assert_eq(1, 1>0, "1>0");
    assert_eq(0, 1>1, "1>1");
    assert_eq(0, 1>2, "1>2");
    assert_eq(1, 1>=0, "1>=0");
    assert_eq(1, 1>=1, "1>=1");
    assert_eq(0, 1>=2, "1>=2");

    // Compound assignment
    int i = 2; i += 5; assert_eq(7, i, "i+=5");
    i = 5; i -= 2; assert_eq(3, i, "i-=2");
    i = 3; i *= 2; assert_eq(6, i, "i*=2");
    i = 6; i /= 3; assert_eq(2, i, "i/=3");

    // Bitwise
    assert_eq(0, 0&1, "0&1");
    assert_eq(3, 7&3, "7&3");
    assert_eq(7, 5|3, "5|3");
    assert_eq(6, 5^3, "5^3");

    // Shift
    assert_eq(1, 1<<0, "1<<0");
    assert_eq(8, 1<<3, "1<<3");
    assert_eq(4, 16>>2, "16>>2");
    assert_eq(2, 8>>2, "8>>2");

    // Logical
    assert_eq(1, 1&&1, "1&&1");
    assert_eq(0, 1&&0, "1&&0");
    assert_eq(0, 0&&1, "0&&1");
    assert_eq(0, 0&&0, "0&&0");
    assert_eq(1, 1||0, "1||0");
    assert_eq(1, 0||1, "0||1");
    assert_eq(0, 0||0, "0||0");

    // Not
    assert_eq(0, !1, "!1");
    assert_eq(1, !0, "!0");
    assert_eq(0, !42, "!42");

    // Bit not
    assert_eq(-1, ~0, "~0");
    assert_eq(-2, ~1, "~1");

    // Ternary
    assert_eq(2, 1?2:3, "1?2:3");
    assert_eq(3, 0?2:3, "0?2:3");

    // Comma
    assert_eq(3, (1,2,3), "(1,2,3)");

    // Sizeof
    assert_eq(4, sizeof(int), "sizeof int");
    assert_eq(8, sizeof(long), "sizeof long");
    assert_eq(1, sizeof(char), "sizeof char");
    assert_eq(8, sizeof(int *), "sizeof ptr");

    // Pre/post inc/dec
    int a = 5;
    assert_eq(6, ++a, "++a");
    assert_eq(6, a++, "a++");
    assert_eq(7, a, "a after a++");
    assert_eq(6, --a, "--a");
    assert_eq(6, a--, "a--");
    assert_eq(5, a, "a after a--");

    test_summary();
    return 0;
}
