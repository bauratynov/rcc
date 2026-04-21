#!/bin/bash
# Benchmark: rcc vs chibicc compilation speed
# Usage: bash bench/bench.sh

set -e

RCC="$(dirname "$0")/../target/release/rcc.exe"
CHIBICC="$(dirname "$0")/../../chibicc/chibicc"
TEST_DIR="$(dirname "$0")/../test_inputs"

echo "=== rcc vs chibicc Benchmark ==="
echo ""

# Build rcc in release mode
echo "Building rcc (release)..."
cd "$(dirname "$0")/.." && cargo build --release 2>/dev/null
echo ""

# Generate a large test file
BIGFILE="/tmp/rcc_bench_big.c"
cat > "$BIGFILE" << 'BIGEOF'
int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }
int mul(int a, int b) { return a * b; }

int fib(int n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

int factorial(int n) {
    int result = 1;
    for (int i = 2; i <= n; i++) {
        result = result * i;
    }
    return result;
}

int sum_to(int n) {
    int s = 0;
    int i = 0;
    while (i <= n) {
        s = s + i;
        i++;
    }
    return s;
}

int gcd(int a, int b) {
    while (b != 0) {
        int t = b;
        b = a % b;
        a = t;
    }
    return a;
}

int main() {
    int a = add(10, 20);
    int b = sub(30, 10);
    int c = mul(5, 6);
    int f = fib(10);
    int fact = factorial(10);
    int s = sum_to(100);
    int g = gcd(48, 36);

    if (a == 30 && b == 20 && c == 30 && f == 55
        && fact == 3628800 && s == 5050 && g == 12) {
        return 0;
    }
    return 1;
}
BIGEOF

echo "--- Compilation Speed (100 iterations) ---"
echo ""

# Benchmark rcc
echo -n "rcc:     "
START=$(date +%s%N)
for i in $(seq 1 100); do
    "$RCC" "$BIGFILE" -o /dev/null 2>/dev/null
done
END=$(date +%s%N)
RCC_TIME=$(( (END - START) / 1000000 ))
echo "${RCC_TIME}ms total, $(( RCC_TIME / 100 ))ms/iteration"

echo ""
echo "--- Output Size ---"
"$RCC" "$BIGFILE" -o /tmp/rcc_out.s 2>/dev/null
RCC_SIZE=$(wc -c < /tmp/rcc_out.s)
echo "rcc asm output: ${RCC_SIZE} bytes"

echo ""
echo "--- Memory Usage (peak RSS) ---"
/usr/bin/time -v "$RCC" "$BIGFILE" -o /dev/null 2>&1 | grep "Maximum resident" || echo "(not available on this platform)"

echo ""
echo "--- Error Message Quality ---"
echo "Test: typo in keyword 'retrun'"
cat > /tmp/rcc_error_test.c << 'EOF'
int main() {
    retrun 0;
}
EOF
echo ""
echo "rcc output:"
"$RCC" /tmp/rcc_error_test.c 2>&1 || true

echo ""
echo "=== Benchmark complete ==="
