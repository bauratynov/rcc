#include <stdio.h>
#include <stdlib.h>

int fail_count = 0;
void check(int cond, char *msg) { if (!cond) { printf("FAIL: %s\n", msg); fail_count = fail_count + 1; } }

int main() {
    // Basic pointer
    int x = 42;
    int *p = &x;
    check(*p == 42, "deref");
    *p = 100;
    check(x == 100, "deref write");

    // Pointer to pointer
    int **pp = &p;
    check(**pp == 100, "ptr to ptr");

    // Array and pointer
    int arr[] = {10, 20, 30, 40, 50};
    check(arr[0] == 10, "arr[0]");
    check(arr[4] == 50, "arr[4]");

    int sum = 0;
    for (int i = 0; i < 5; i++) sum = sum + arr[i];
    check(sum == 150, "arr sum");

    // Malloc
    int *m = (int *)malloc(8);
    *m = 999;
    check(*m == 999, "malloc");
    free(m);

    // String
    char *s = "hello";
    check(s[0] == 'h', "str[0]");
    check(s[4] == 'o', "str[4]");

    if (fail_count == 0) {
        printf("ALL %d pointer tests PASSED\n", 9);
    } else {
        printf("%d pointer tests FAILED\n", fail_count);
    }
    return fail_count;
}
