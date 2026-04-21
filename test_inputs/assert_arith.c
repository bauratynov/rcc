#include <stdio.h>

int fail_count = 0;

void check(int cond, char *msg) {
    if (!cond) {
        printf("FAIL: %s\n", msg);
        fail_count = fail_count + 1;
    }
}

int main() {
    // Basic arithmetic
    check(1 + 2 == 3, "1+2==3");
    check(10 - 3 == 7, "10-3==7");
    check(3 * 4 == 12, "3*4==12");
    check(10 / 3 == 3, "10/3==3");
    check(10 % 3 == 1, "10%3==1");
    check(-5 + 5 == 0, "-5+5==0");
    check(2 + 3 * 4 == 14, "precedence");
    check((2 + 3) * 4 == 20, "parens");

    // Comparison
    check(1 < 2, "1<2");
    check(2 > 1, "2>1");
    check(1 <= 1, "1<=1");
    check(1 >= 1, "1>=1");
    check(1 == 1, "1==1");
    check(1 != 2, "1!=2");

    // Bitwise
    check((0xFF & 0x0F) == 0x0F, "and");
    check((0xF0 | 0x0F) == 0xFF, "or");
    check((0xFF ^ 0x0F) == 0xF0, "xor");
    check((1 << 3) == 8, "shl");
    check((16 >> 2) == 4, "shr");
    check(~0 == -1, "bitnot");

    // Logical
    check(1 && 1, "logand true");
    check(!(0 && 1), "logand false");
    check(1 || 0, "logor true");
    check(!(0 || 0), "logor false");
    check(!0, "not 0");
    check(!!1, "not not 1");

    // Unary
    check(-(-5) == 5, "neg neg");
    // unary + not supported yet

    // Ternary
    check((1 ? 10 : 20) == 10, "ternary true");
    check((0 ? 10 : 20) == 20, "ternary false");

    // Sizeof
    check(sizeof(int) == 4, "sizeof int");
    check(sizeof(long) == 8, "sizeof long");
    check(sizeof(char) == 1, "sizeof char");

    // Compound assignment
    int x = 10;
    x += 5; check(x == 15, "+=");
    x -= 3; check(x == 12, "-=");
    x *= 2; check(x == 24, "*=");
    x /= 6; check(x == 4, "/=");
    x %= 3; check(x == 1, "%=");

    // Pre/post increment
    int a = 5;
    check(++a == 6, "pre inc");
    check(a++ == 6, "post inc");
    check(a == 7, "after post inc");
    check(--a == 6, "pre dec");
    check(a-- == 6, "post dec");
    check(a == 5, "after post dec");

    if (fail_count == 0) {
        printf("ALL %d arithmetic tests PASSED\n", 42);
    } else {
        printf("%d arithmetic tests FAILED\n", fail_count);
    }
    return fail_count;
}
