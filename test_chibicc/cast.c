#include "test.h"

int main() {
    // Int casts
    assert_eq(42, (int)42, "(int)42");
    assert_eq(42, (long)42, "(long)42");

    // Float casts
    assert_eq(3, (int)3.7, "(int)3.7");
    assert_eq(0, (int)0.9, "(int)0.9");

    // Pointer cast
    long x = 42;
    long *p = &x;
    long addr = (long)p;
    assert_eq(1, addr != 0, "ptr to long");

    // Void pointer
    void *vp = (void *)p;
    long *p2 = (long *)vp;
    assert_eq(42, (int)*p2, "void* round trip");

    test_summary();
    return 0;
}
