#include <stdio.h>

int main() {
    int arr[5];
    arr[0] = 10;
    arr[1] = 20;
    arr[2] = 30;
    arr[3] = 40;
    arr[4] = 50;

    int sum = 0;
    for (int i = 0; i < 5; i++) {
        sum = sum + arr[i];
    }

    printf("sum = %d\n", sum);
    if (sum == 150) {
        return 0;
    }
    return 1;
}
