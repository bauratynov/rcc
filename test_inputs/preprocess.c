#define MAX 100
#define ADD(a, b) ((a) + (b))

#ifdef MAX
int has_max = 1;
#else
int has_max = 0;
#endif

#ifndef UNDEFINED_THING
int not_defined = 1;
#endif

int main() {
    int x = MAX;
    int y = ADD(10, 20);
    if (x == 100 && y == 30 && has_max == 1 && not_defined == 1) {
        return 0;
    }
    return 1;
}
