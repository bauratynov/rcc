#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef int cJSON_bool;

typedef struct {
    void *(*allocate)(long long);
    void (*deallocate)(void *);
    void *(*reallocate)(void *, long long);
} internal_hooks;

static internal_hooks global_hooks = { malloc, free, realloc };

typedef struct cJSON {
    struct cJSON *next;
    struct cJSON *prev;
    struct cJSON *child;
    int type;
    char *valuestring;
    int valueint;
    double valuedouble;
    char *string;
} cJSON;

typedef struct {
    char *content;
    long long length;
    long long offset;
    long long depth;
    internal_hooks hooks;
} parse_buffer;

static cJSON *cJSON_New_Item(internal_hooks *hooks) {
    cJSON *node = (cJSON *)hooks->allocate(sizeof(cJSON));
    if (node) {
        char *p = (char *)node;
        for (int i = 0; i < sizeof(cJSON); i++) p[i] = 0;
    }
    return node;
}

static cJSON_bool parse_value(cJSON *item, parse_buffer *buf) {
    if (buf == 0 || buf->content == 0) return 0;

    if ((buf->offset + 4) <= buf->length &&
        strncmp(buf->content + buf->offset, "null", 4) == 0) {
        item->type = 4;
        buf->offset = buf->offset + 4;
        return 1;
    }
    if ((buf->offset + 4) <= buf->length &&
        strncmp(buf->content + buf->offset, "true", 4) == 0) {
        item->type = 2;
        item->valueint = 1;
        buf->offset = buf->offset + 4;
        return 1;
    }
    if (buf->offset < buf->length && buf->content[buf->offset] == '"') {
        // Simple string parse
        item->type = 16;
        long long start = buf->offset + 1;
        long long end = start;
        while (end < buf->length && buf->content[end] != '"') end++;
        long long len = end - start;
        char *s = (char *)global_hooks.allocate(len + 1);
        for (long long i = 0; i < len; i++) s[i] = buf->content[start + i];
        s[len] = 0;
        item->valuestring = s;
        buf->offset = end + 1;
        return 1;
    }
    if (buf->offset < buf->length && buf->content[buf->offset] == '{') {
        item->type = 64;
        buf->offset = buf->offset + 1;
        // Skip simple object content for test
        while (buf->offset < buf->length && buf->content[buf->offset] != '}')
            buf->offset = buf->offset + 1;
        if (buf->offset < buf->length) buf->offset = buf->offset + 1;
        return 1;
    }
    return 0;
}

cJSON *mini_parse(char *json) {
    parse_buffer buf = { 0, 0, 0, 0, { 0, 0, 0 } };
    buf.content = json;
    buf.length = strlen(json);
    buf.offset = 0;
    buf.hooks = global_hooks;

    cJSON *item = cJSON_New_Item(&global_hooks);
    if (item == 0) return 0;

    if (!parse_value(item, &buf)) {
        global_hooks.deallocate(item);
        return 0;
    }
    return item;
}

int main() {
    cJSON *s = mini_parse("\"hello\"");
    if (s != 0 && s->type == 16) {
        printf("string: type=%d val=%s\n", s->type, s->valuestring);
        global_hooks.deallocate(s->valuestring);
        global_hooks.deallocate(s);
    } else { printf("string FAIL\n"); return 1; }

    cJSON *t = mini_parse("true");
    if (t != 0 && t->type == 2) {
        printf("true: type=%d\n", t->type);
        global_hooks.deallocate(t);
    } else { printf("true FAIL\n"); return 1; }

    printf("mini cJSON PASSED!\n");
    return 0;
}
