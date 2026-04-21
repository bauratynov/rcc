#include "test.h"

typedef int i32;
typedef long long i64;
typedef char *string;

typedef struct Point { long x; long y; } Point;

i32 add_i32(i32 a, i32 b) { return a + b; }

int main() {
    i32 x = 42;
    assert_eq(42, x, "i32");

    i64 big = 1000000;
    assert_eq(1000000, (int)big, "i64");

    string s = "hello";
    assert_eq('h', s[0], "string typedef");

    assert_eq(15, add_i32(10, 5), "add_i32");

    Point p;
    p.x = 100; p.y = 200;
    assert_eq(300, (int)(p.x + p.y), "Point typedef");

    assert_eq(4, sizeof(i32), "sizeof i32");
    assert_eq(8, sizeof(i64), "sizeof i64");
    assert_eq(16, sizeof(Point), "sizeof Point");

    test_summary();
    return 0;
}
