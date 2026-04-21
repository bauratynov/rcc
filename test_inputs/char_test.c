#include <stdio.h>

int main() {
    char c = 'A';
    printf("char: %c (%d)\n", c, c);

    char *msg = "Hello rcc!";
    printf("msg: %s\n", msg);
    printf("msg[0]=%c msg[5]=%c\n", msg[0], msg[5]);

    return 0;
}
