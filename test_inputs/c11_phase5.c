#include <stdio.h>

#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define SQUARE(x) ((x) * (x))

enum Direction { UP = 0, DOWN = 1, LEFT = 2, RIGHT = 3 };

int main() {
    // Cast
    int x = (int)3.14;

    // Function-like macros
    int m = MAX(10, 20);
    int sq = SQUARE(5);

    // Enum
    int dir = LEFT;

    // Multiple declarations
    int a = 1, b = 2, c = 3;

    printf("x=%d m=%d sq=%d dir=%d\n", x, m, sq, dir);
    printf("a=%d b=%d c=%d\n", a, b, c);

    if (x == 3 && m == 20 && sq == 25 && dir == 2 && a + b + c == 6) {
        return 0;
    }
    return 1;
}
