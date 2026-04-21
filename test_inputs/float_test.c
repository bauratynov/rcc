#include <stdio.h>

int main() {
    double a = 3.14;
    double b = 2.0;
    double c = a + b;
    double d = a * b;
    double e = a - b;
    double f = a / b;

    printf("a = %f\n", a);
    printf("b = %f\n", b);
    printf("a + b = %f\n", c);
    printf("a * b = %f\n", d);
    printf("a - b = %f\n", e);
    printf("a / b = %f\n", f);

    // Int to double conversion
    int x = 42;
    double g = (double)x;
    printf("(double)42 = %f\n", g);

    // Double to int
    int y = (int)3.7;
    printf("(int)3.7 = %d\n", y);

    // Comparison
    if (a > b) {
        printf("3.14 > 2.0: YES\n");
    }

    return 0;
}
