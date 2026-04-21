#include "test.h"

int main() {
    // Basic pointer
    int x = 3;
    int *p = &x;
    assert_eq(3, *p, "*p");
    *p = 5;
    assert_eq(5, x, "*p=5");

    // Pointer arithmetic
    int arr[] = {10, 20, 30, 40, 50};
    int *q = arr;
    assert_eq(10, *q, "arr[0]");
    assert_eq(10, arr[0], "arr[0] index");
    assert_eq(30, arr[2], "arr[2]");
    assert_eq(50, arr[4], "arr[4]");

    // Array sum
    int sum = 0;
    for (int i = 0; i < 5; i++) sum += arr[i];
    assert_eq(150, sum, "arr sum");

    // Sizeof array
    assert_eq(20, sizeof(arr), "sizeof arr");
    assert_eq(4, sizeof(arr[0]), "sizeof arr[0]");

    // Pointer to pointer
    int **pp = &p;
    assert_eq(5, **pp, "**pp");

    // String
    char *s = "hello";
    assert_eq('h', s[0], "s[0]");
    assert_eq('o', s[4], "s[4]");
    assert_eq(0, s[5], "s[5] null");

    // Null pointer
    int *null = 0;
    assert_eq(1, null == 0, "null == 0");
    assert_eq(0, null != 0, "null != 0");

    test_summary();
    return 0;
}
