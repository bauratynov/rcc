int main() {
    int sum = 0;
    int i = 0;
    while (i < 10) {
        sum = sum + i;
        i++;
    }

    int fact = 1;
    for (int j = 1; j <= 5; j++) {
        fact = fact * j;
    }

    int k = 3;
    do {
        k--;
    } while (k > 0);

    if (sum == 45 && fact == 120 && k == 0) {
        return 0;
    }
    return 1;
}
