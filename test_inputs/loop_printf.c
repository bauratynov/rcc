#include <stdio.h>

int main() {
    int sum = 0;
    for (int i = 0; i < 5; i++) {
        printf("i=%d\n", i);
        sum = sum + i;
    }
    printf("sum=%d\n", sum);
    if (sum == 10) {
        return 0;
    }
    return 1;
}
