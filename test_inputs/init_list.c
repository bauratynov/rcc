#include <stdio.h>

int main() {
    int arr[] = {10, 20, 30, 40, 50};

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
