#include <stdio.h>

typedef int i32;
typedef long long i64;
typedef unsigned int u32;

i32 add(i32 a, i32 b) {
    return a + b;
}

int main() {
    i32 x = 10;
    i64 big = 1000000;
    i32 result = add(x, 20);

    printf("x=%d big=%lld result=%d\n", x, big, result);
    printf("sizeof(i32)=%d sizeof(i64)=%d\n", sizeof(i32), sizeof(i64));

    if (result == 30) {
        return 0;
    }
    return 1;
}
