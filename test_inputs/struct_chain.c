#include <stdio.h>
#include <stdlib.h>

typedef struct Node {
    int value;
    struct Node *next;
} Node;

int main() {
    Node a;
    Node b;
    a.value = 10;
    a.next = &b;
    b.value = 20;
    b.next = 0;

    printf("a.value=%d\n", a.value);
    printf("a.next->value=%d\n", a.next->value);

    int sum = a.value + a.next->value;
    printf("sum=%d\n", sum);

    if (sum == 30) {
        return 0;
    }
    return 1;
}
