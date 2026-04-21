#include <stdio.h>
#include <stdlib.h>

int fail_count = 0;
void check(int cond, char *msg) { if (!cond) { printf("FAIL: %s\n", msg); fail_count = fail_count + 1; } }

typedef struct Point { long x; long y; } Point;

typedef struct Node {
    long value;
    struct Node *next;
} Node;

long point_sum(Point p) { return p.x + p.y; }

int main() {
    // Basic struct
    Point p;
    p.x = 10;
    p.y = 20;
    check(p.x == 10, "p.x");
    check(p.y == 20, "p.y");
    check(p.x + p.y == 30, "p.x+p.y");

    // Struct pointer
    Point *pp = &p;
    check(pp->x == 10, "pp->x");
    check(pp->y == 20, "pp->y");
    pp->x = 100;
    check(p.x == 100, "pp->x write");

    // Linked list
    Node a; a.value = 1; a.next = 0;
    Node b; b.value = 2; b.next = &a;
    Node c; c.value = 3; c.next = &b;

    check(c.next->value == 2, "chain 1");
    check(c.next->next->value == 1, "chain 2");

    // Malloc struct
    Point *mp = (Point *)malloc(sizeof(Point));
    mp->x = 42;
    mp->y = 58;
    check(mp->x + mp->y == 100, "malloc struct");
    free(mp);

    // Sizeof
    check(sizeof(Point) == 16, "sizeof Point");

    // Enum
    check(sizeof(long) == 8, "sizeof long");

    if (fail_count == 0) {
        printf("ALL %d struct tests PASSED\n", 12);
    } else {
        printf("%d struct tests FAILED\n", fail_count);
    }
    return fail_count;
}
