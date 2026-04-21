#include "test.h"

int main() {
    // String literals
    assert_eq('h', "hello"[0], "str[0]");
    assert_eq('o', "hello"[4], "str[4]");
    assert_eq(0, "hello"[5], "str null");

    // String concatenation
    char *s = "hello" " world";
    assert_eq('w', s[6], "concat");

    // Escape sequences
    assert_eq(10, '\n', "\\n");
    assert_eq(9, '\t', "\\t");
    assert_eq(0, '\0', "\\0");
    assert_eq(92, '\\', "\\\\");
    assert_eq(39, '\'', "\\'");

    // Strlen
    assert_eq(5, (int)strlen("hello"), "strlen");
    assert_eq(0, (int)strlen(""), "strlen empty");

    // Strcmp
    assert_eq(0, strcmp("abc", "abc"), "strcmp eq");
    assert_eq(1, strcmp("abc", "abc") == 0, "strcmp eq bool");
    assert_eq(0, strcmp("abc", "abd") == 0, "strcmp ne");

    // Hex/octal escape
    assert_eq(65, '\x41', "hex escape");
    assert_eq(10, '\012', "octal escape");

    test_summary();
    return 0;
}
