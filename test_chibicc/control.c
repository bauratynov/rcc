#include "test.h"

int fib(int n) { if (n<=1) return n; return fib(n-1)+fib(n-2); }
int fact(int n) { int r=1; for(int i=2;i<=n;i++) r*=i; return r; }

int main() {
    // If
    int x = 0;
    if (1) x = 1;
    assert_eq(1, x, "if true");
    if (0) x = 99;
    assert_eq(1, x, "if false");
    if (0) x = 99; else x = 2;
    assert_eq(2, x, "if-else");

    // While
    int sum = 0; int i = 0;
    while (i < 10) { sum += i; i++; }
    assert_eq(45, sum, "while sum");

    // For
    sum = 0;
    for (int j = 1; j <= 5; j++) sum += j;
    assert_eq(15, sum, "for sum");

    // Do-while
    int k = 0;
    do { k++; } while (k < 3);
    assert_eq(3, k, "do-while");

    // Break
    sum = 0;
    for (int j = 0; j < 100; j++) { if (j==5) break; sum += j; }
    assert_eq(10, sum, "break");

    // Continue
    sum = 0;
    for (int j = 0; j < 10; j++) { if (j%2==0) continue; sum += j; }
    assert_eq(25, sum, "continue");

    // Switch
    int r = 0;
    switch (2) { case 1: r=10; break; case 2: r=20; break; case 3: r=30; break; default: r=-1; break; }
    assert_eq(20, r, "switch 2");
    switch (9) { case 1: r=10; break; default: r=-1; break; }
    assert_eq(-1, r, "switch default");

    // Goto
    goto label1;
    r = 999;
    label1:
    assert_eq(-1, r, "goto");

    // Recursion
    assert_eq(55, fib(10), "fib(10)");
    assert_eq(120, fact(5), "fact(5)");

    // Nested if
    if (1) { if (1) { x = 100; } else { x = 200; } } else { x = 300; }
    assert_eq(100, x, "nested if");

    test_summary();
    return 0;
}
