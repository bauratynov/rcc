int main() {
    int x = 42;
    int *p = &x;
    *p = 100;

    int y = *p;

    if (y == 100) {
        return 0;
    }
    return 1;
}
