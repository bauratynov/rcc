int double_it(int x) {
    return x * 2;
}

int add_one(int x) {
    return x + 1;
}

int main() {
    int result = double_it(add_one(double_it(5)));
    // double_it(5) = 10, add_one(10) = 11, double_it(11) = 22
    if (result == 22) {
        return 0;
    }
    return 1;
}
