#include <stdio.h>
#include <stdlib.h>

int main() {
    char *str = "3.14";
    char *end;
    double d = strtod(str, &end);
    printf("strtod(\"%s\") = %f\n", str, d);

    char *str2 = "42";
    double d2 = strtod(str2, &end);
    printf("strtod(\"%s\") = %f\n", str2, d2);

    char *str3 = "-1.5e10";
    double d3 = strtod(str3, &end);
    printf("strtod(\"%s\") = %f\n", str3, d3);

    return 0;
}
