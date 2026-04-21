#include <stdio.h>
#include <stdlib.h>

typedef struct Node {
    struct Node *child;
    int type;
    char *name;
} Node;

void add_child(Node *parent, Node *child) {
    parent->child = child;
}

int main() {
    Node *parent = (Node *)malloc(sizeof(Node));
    Node *child = (Node *)malloc(sizeof(Node));

    parent->child = 0;
    parent->type = 1;
    parent->name = 0;

    child->child = 0;
    child->type = 2;
    child->name = "hello";

    printf("Before: parent->child = %p\n", parent->child);
    add_child(parent, child);
    printf("After:  parent->child = %p\n", parent->child);

    if (parent->child != 0) {
        printf("child->name = %s\n", parent->child->name);
        printf("PASSED\n");
        return 0;
    }
    printf("FAILED\n");
    return 1;
}
