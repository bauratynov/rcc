#include <stdio.h>
#include <string.h>

int main() {
    char *s = "Hello";
    long len = strlen(s);
    printf("strlen(\"%s\") = %d\n", s, len);

    int cmp = strcmp("abc", "abc");
    printf("strcmp(\"abc\", \"abc\") = %d\n", cmp);

    if (len == 5 && cmp == 0) {
        printf("All string tests passed!\n");
        return 0;
    }
    return 1;
}
