#include <stdio.h>
#include <stdlib.h>

typedef struct Node {
    struct Node *next;
    struct Node *child;
    int type;
    char *name;
} Node;

void add_child(Node *parent, Node *new_child) {
    if (parent->child == 0) {
        parent->child = new_child;
    } else {
        Node *last = parent->child;
        while (last->next != 0) {
            last = last->next;
        }
        last->next = new_child;
    }
}

int main() {
    Node *root = (Node *)malloc(sizeof(Node));
    root->next = 0;
    root->child = 0;
    root->type = 1;
    root->name = 0;

    Node *c1 = (Node *)malloc(sizeof(Node));
    c1->next = 0;
    c1->child = 0;
    c1->type = 2;
    c1->name = "first";

    Node *c2 = (Node *)malloc(sizeof(Node));
    c2->next = 0;
    c2->child = 0;
    c2->type = 3;
    c2->name = "second";

    add_child(root, c1);
    add_child(root, c2);

    printf("root->child = %p\n", root->child);
    if (root->child != 0) {
        printf("first = %s\n", root->child->name);
        if (root->child->next != 0) {
            printf("second = %s\n", root->child->next->name);
        }
    }

    if (root->child != 0 && root->child->next != 0) {
        printf("PASSED\n");
        return 0;
    }
    printf("FAILED\n");
    return 1;
}
