#include <stdio.h>
#include <string.h>

typedef struct {
    char *content;
    long long length;
    long long offset;
} Buffer;

int parse_value(int *out_type, Buffer *buf) {
    if (buf == 0 || buf->content == 0) {
        return 0;
    }

    // Pattern from cJSON parse_value:
    // can_read(buffer, 4) && strncmp(buffer_at_offset, "null", 4) == 0
    if ((buf->offset + 4) <= buf->length &&
        strncmp(buf->content + buf->offset, "null", 4) == 0) {
        *out_type = 4;
        buf->offset = buf->offset + 4;
        return 1;
    }

    if ((buf->offset + 5) <= buf->length &&
        strncmp(buf->content + buf->offset, "false", 5) == 0) {
        *out_type = 1;
        buf->offset = buf->offset + 5;
        return 1;
    }

    if ((buf->offset + 4) <= buf->length &&
        strncmp(buf->content + buf->offset, "true", 4) == 0) {
        *out_type = 2;
        buf->offset = buf->offset + 4;
        return 1;
    }

    // String: starts with "
    if (buf->offset < buf->length &&
        (buf->content + buf->offset)[0] == '"') {
        *out_type = 16;
        return 1;
    }

    // Number: starts with digit or -
    char c = (buf->content + buf->offset)[0];
    if (c == '-' || (c >= '0' && c <= '9')) {
        *out_type = 8;
        return 1;
    }

    // Object: starts with {
    if (c == '{') {
        *out_type = 64;
        return 1;
    }

    return 0;
}

int main() {
    int type;
    Buffer buf;

    // Test null
    buf.content = "null"; buf.length = 4; buf.offset = 0;
    type = 0;
    if (parse_value(&type, &buf)) {
        printf("null: type=%d offset=%d\n", type, (int)buf.offset);
    } else { printf("null: FAIL\n"); }

    // Test true
    buf.content = "true"; buf.length = 4; buf.offset = 0;
    type = 0;
    if (parse_value(&type, &buf)) {
        printf("true: type=%d\n", type);
    } else { printf("true: FAIL\n"); }

    // Test false
    buf.content = "false"; buf.length = 5; buf.offset = 0;
    type = 0;
    if (parse_value(&type, &buf)) {
        printf("false: type=%d\n", type);
    } else { printf("false: FAIL\n"); }

    // Test string
    buf.content = "\"hello\""; buf.length = 7; buf.offset = 0;
    type = 0;
    if (parse_value(&type, &buf)) {
        printf("string: type=%d\n", type);
    } else { printf("string: FAIL\n"); }

    // Test number
    buf.content = "42"; buf.length = 2; buf.offset = 0;
    type = 0;
    if (parse_value(&type, &buf)) {
        printf("number: type=%d\n", type);
    } else { printf("number: FAIL\n"); }

    // Test object
    buf.content = "{\"x\":1}"; buf.length = 7; buf.offset = 0;
    type = 0;
    if (parse_value(&type, &buf)) {
        printf("object: type=%d\n", type);
    } else { printf("object: FAIL\n"); }

    if (type == 64) { printf("PASSED\n"); return 0; }
    return 1;
}
