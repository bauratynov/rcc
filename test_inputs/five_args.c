#include <stdio.h>

int add5(int a, int b, int c, int d, int e) {
    return a + b + c + d + e;
}

int main() {
    int result = add5(1, 2, 3, 4, 5);
    printf("result = %d\n", result);
    if (result == 15) {
        return 0;
    }
    return 1;
}
