int main() {
    int a = 10;
    int b = 3;

    int add = a + b;
    int sub = a - b;
    int mul = a * b;
    int div = a / b;
    int mod = a % b;

    int neg = -a;
    int not = !0;
    int bnot = ~0;

    int shl = 1 << 3;
    int shr = 16 >> 2;

    int band = 0xFF & 0x0F;
    int bor = 0xF0 | 0x0F;
    int bxor = 0xFF ^ 0x0F;

    if (add == 13 && sub == 7 && mul == 30 && div == 3 && mod == 1
        && neg == -10 && not == 1 && shl == 8 && shr == 4
        && band == 15 && bor == 255 && bxor == 240) {
        return 0;
    }
    return 1;
}
