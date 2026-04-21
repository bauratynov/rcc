#include <stdio.h>
#include <stdlib.h>
#include <string.h>

long _test_count = 0;
long _test_fail = 0;

void assert_eq(long expected, long actual, char *msg) {
    _test_count = _test_count + 1;
    if (expected != actual) {
        printf("FAIL: %s => expected %d, got %d\n", msg, (int)expected, (int)actual);
        _test_fail = _test_fail + 1;
    }
}

void test_summary() {
    if (_test_fail == 0) {
        printf("ALL %d tests PASSED\n", (int)_test_count);
    } else {
        printf("%d/%d tests FAILED\n", (int)_test_fail, (int)_test_count);
        exit(1);
    }
}
