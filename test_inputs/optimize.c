int main() {
    // Constant folding: should be computed at compile time
    int a = 2 + 3 * 4;        // should become 14
    int b = (10 - 2) / 4;     // should become 2
    int c = 1 << 3;            // should become 8

    // Dead code: if(0) branch should be eliminated
    if (0) {
        return 99;
    }

    // Strength reduction: x * 8 should become x << 3
    int x = 5;
    int y = x * 8;

    // Identity: x + 0, x * 1 should be simplified
    int z = a + 0;
    int w = b * 1;

    if (a == 14 && b == 2 && c == 8) {
        return 0;
    }
    return 1;
}
