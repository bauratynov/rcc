#include <stdio.h>

enum Color { RED, GREEN = 5, BLUE };

int main() {
    // Multiple declarations
    int a = 1, b = 2, c = 3;

    // Enum values
    int r = RED;
    int g = GREEN;
    int bl = BLUE;

    // String concatenation
    char *msg = "Hello" " " "World";

    // Array initializer
    int arr[] = {10, 20, 30};

    printf("a=%d b=%d c=%d\n", a, b, c);
    printf("RED=%d GREEN=%d BLUE=%d\n", r, g, bl);
    printf("msg=%s\n", msg);
    printf("arr=%d,%d,%d\n", arr[0], arr[1], arr[2]);

    if (a == 1 && b == 2 && c == 3
        && r == 0 && g == 5 && bl == 6) {
        return 0;
    }
    return 1;
}
