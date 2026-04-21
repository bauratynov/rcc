#include <stdio.h>
#include <stdlib.h>

int main() {
    char buf[64];
    double d = 3.14;

    sprintf(buf, "%f", d);
    printf("sprintf %%f: %s\n", buf);

    sprintf(buf, "%g", d);
    printf("sprintf %%g: %s\n", buf);

    sprintf(buf, "%1.17g", d);
    printf("sprintf %%1.17g: %s\n", buf);

    return 0;
}
