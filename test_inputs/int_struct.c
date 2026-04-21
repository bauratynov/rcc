#include <stdio.h>

struct Point {
    int x;
    int y;
};

int main() {
    struct Point p;
    p.x = 10;
    p.y = 20;
    int sum = p.x + p.y;
    printf("p.x=%d p.y=%d sum=%d\n", p.x, p.y, sum);
    if (sum == 30) {
        return 0;
    }
    return 1;
}
