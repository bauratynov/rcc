#include <stdio.h>

int fail_count = 0;
void check(int cond, char *msg) { if (!cond) { printf("FAIL: %s\n", msg); fail_count = fail_count + 1; } }

int fib(int n) { if (n <= 1) return n; return fib(n-1) + fib(n-2); }

int main() {
    // If/else
    int x = 0;
    if (1) x = 1;
    check(x == 1, "if true");
    if (0) x = 99; else x = 2;
    check(x == 2, "if-else");

    // While
    int sum = 0;
    int i = 0;
    while (i < 10) { sum = sum + i; i++; }
    check(sum == 45, "while sum");

    // For
    sum = 0;
    for (int j = 1; j <= 5; j++) { sum = sum + j; }
    check(sum == 15, "for sum");

    // Do-while
    int k = 0;
    do { k++; } while (k < 3);
    check(k == 3, "do-while");

    // Break
    sum = 0;
    for (int j = 0; j < 100; j++) { if (j == 5) break; sum = sum + j; }
    check(sum == 10, "break");

    // Continue
    sum = 0;
    for (int j = 0; j < 10; j++) { if (j % 2 == 0) continue; sum = sum + j; }
    check(sum == 25, "continue");

    // Switch
    int r = 0;
    switch (2) { case 1: r = 10; break; case 2: r = 20; break; case 3: r = 30; break; }
    check(r == 20, "switch");

    // Recursion
    check(fib(0) == 0, "fib(0)");
    check(fib(1) == 1, "fib(1)");
    check(fib(10) == 55, "fib(10)");

    // Nested calls
    check(fib(fib(5)) == 5, "nested fib");

    if (fail_count == 0) {
        printf("ALL %d control flow tests PASSED\n", 15);
    } else {
        printf("%d control flow tests FAILED\n", fail_count);
    }
    return fail_count;
}
