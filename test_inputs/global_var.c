int counter = 0;

void increment() {
    counter = counter + 1;
}

int main() {
    increment();
    increment();
    increment();
    if (counter == 3) {
        return 0;
    }
    return 1;
}
