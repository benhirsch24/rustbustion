#include <stdio.h>
#include <stdint.h>

struct __attribute__((packed)) Temp {
	unsigned int t1:13;
	unsigned int t2:13;
	unsigned int t3:13;
	unsigned int t4:13;
	unsigned int t5:13;
	unsigned int t6:13;
	unsigned int t7:13;
	unsigned int t8:13;
};

int main(int argc, char **argv) {
	//uint8_t ar[13] = {0x55, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,};
	uint8_t ar[13] = {0x37, 0xc3, 0x64, 0x74, 0x8c, 0x8a, 0xf1, 0x30, 0x10, 0x06, 0xc2, 0x20, 0x18};

	struct Temp *t = (struct Temp*)ar;
	printf("%d %f\n", t->t1, ((float) (t->t1)) * 0.05 - 20.0 );
	printf("%d %f\n", t->t2, ((float) (t->t2)) * 0.05 - 20.0 );
	printf("%d %f\n", t->t3, ((float) (t->t3)) * 0.05 - 20.0 );
	printf("%d %f\n", t->t4, ((float) (t->t4)) * 0.05 - 20.0 );
	printf("%d %f\n", t->t5, ((float) (t->t5)) * 0.05 - 20.0 );
	printf("%d %f\n", t->t6, ((float) (t->t6)) * 0.05 - 20.0 );
	printf("%d %f\n", t->t7, ((float) (t->t7)) * 0.05 - 20.0 );
	printf("%d %f\n", t->t8, ((float) (t->t8)) * 0.05 - 20.0 );
	printf("=====\n\n");

	struct Temp x;
	memset(&x, 0, 13);
	x.t1 = 823;
	x.t2 = 5;
	uint8_t *a = &x;
	printf("t1: 0x%x\n", x.t1);
	for (int i = 0; i < 13; i++) {
		printf("0x%x ", a[i]);
	}
	printf("\n");
	return 0;
}
