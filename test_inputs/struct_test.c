#include <stdio.h>

struct Point {
    long x;
    long y;
};

int main() {
    struct Point p;
    p.x = 10;
    p.y = 20;
    long sum = p.x + p.y;
    printf("p.x = %d, p.y = %d, sum = %d\n", p.x, p.y, sum);
    return 0;
}
