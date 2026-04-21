#include <stdio.h>
#include <stdlib.h>

typedef struct {
    void *(*alloc)(long long);
    void (*dealloc)(void *);
    void *(*realloc)(void *, long long);
} Hooks;

typedef struct {
    char *content;
    long long length;
    long long offset;
    long long depth;
    Hooks hooks;
} Buffer;

static Hooks global_hooks = { malloc, free, realloc };

int main() {
    Buffer buf = { 0, 0, 0, 0, { 0, 0, 0 } };

    printf("before: buf.hooks.alloc = %p\n", buf.hooks.alloc);

    buf.content = "hello";
    buf.length = 5;
    buf.hooks = global_hooks;

    printf("after: buf.hooks.alloc = %p\n", buf.hooks.alloc);
    printf("global_hooks.alloc = %p\n", global_hooks.alloc);

    if (buf.hooks.alloc != 0) {
        void *p = buf.hooks.alloc(16);
        if (p != 0) {
            printf("alloc worked: %p\n", p);
            buf.hooks.dealloc(p);
            printf("dealloc worked\n");
            printf("PASSED\n");
            return 0;
        }
    }
    printf("FAILED\n");
    return 1;
}
