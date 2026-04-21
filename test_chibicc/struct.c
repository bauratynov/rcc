#include "test.h"

typedef struct { int a; int b; } Pair;
typedef struct { long x; long y; long z; } Vec3;
typedef struct Node { long val; struct Node *next; } Node;

long pair_sum(Pair *p) { return p->a + p->b; }

int main() {
    // Basic struct
    Pair p;
    p.a = 10; p.b = 20;
    assert_eq(10, p.a, "p.a");
    assert_eq(20, p.b, "p.b");

    // Struct pointer
    Pair *pp = &p;
    assert_eq(10, pp->a, "pp->a");
    assert_eq(20, pp->b, "pp->b");
    pp->a = 100;
    assert_eq(100, p.a, "pp->a write");

    // Function with struct ptr
    assert_eq(120, (int)pair_sum(&p), "pair_sum");

    // Sizeof
    assert_eq(8, sizeof(Pair), "sizeof Pair");
    assert_eq(24, sizeof(Vec3), "sizeof Vec3");

    // Linked list
    Node a; a.val = 1; a.next = 0;
    Node b; b.val = 2; b.next = &a;
    Node c; c.val = 3; c.next = &b;
    assert_eq(2, (int)c.next->val, "c.next->val");
    assert_eq(1, (int)c.next->next->val, "c.next->next->val");

    // Struct init list
    Vec3 v;
    v.x = 1; v.y = 2; v.z = 3;
    assert_eq(6, (int)(v.x + v.y + v.z), "vec sum");

    test_summary();
    return 0;
}
