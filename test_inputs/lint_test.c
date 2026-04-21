int foo() {
    int unused_var = 42;
    int x = 10;
    return x;
    int dead_code = 99;
}

int main() {
    return foo();
}
