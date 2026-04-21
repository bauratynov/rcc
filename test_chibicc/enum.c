#include "test.h"

enum Color { RED, GREEN, BLUE };
enum Weekday { MON = 1, TUE, WED, THU, FRI, SAT = 10, SUN };

int main() {
    assert_eq(0, RED, "RED");
    assert_eq(1, GREEN, "GREEN");
    assert_eq(2, BLUE, "BLUE");

    assert_eq(1, MON, "MON");
    assert_eq(2, TUE, "TUE");
    assert_eq(3, WED, "WED");
    assert_eq(10, SAT, "SAT");
    assert_eq(11, SUN, "SUN");

    int c = GREEN;
    assert_eq(1, c, "enum var");

    switch (c) {
    case RED: assert_eq(0, 1, "should not be RED"); break;
    case GREEN: assert_eq(1, 1, "is GREEN"); break;
    case BLUE: assert_eq(0, 1, "should not be BLUE"); break;
    }

    assert_eq(4, sizeof(enum Color), "sizeof enum");

    test_summary();
    return 0;
}
