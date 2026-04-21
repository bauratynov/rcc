#include <stdio.h>

int add6(int a, int b, int c, int d, int e, int f) {
    return a + b + c + d + e + f;
}

int main() {
    int r = add6(1, 2, 3, 4, 5, 6);
    printf("r = %d\n", r);
    if (r == 21) { return 0; }
    return 1;
}
