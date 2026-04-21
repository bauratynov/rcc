int main() {
    int x = 10;
    int y = x > 5 ? 100 : 200;
    int z = x < 5 ? 100 : 200;

    if (y == 100 && z == 200) {
        return 0;
    }
    return 1;
}
