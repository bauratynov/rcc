#ifndef _STDIO_H
#define _STDIO_H

typedef long long size_t;
typedef void FILE;

int printf(char *fmt, ...);
int puts(char *s);
int putchar(int c);
int getchar(void);
int sprintf(char *str, char *fmt, ...);
int snprintf(char *str, size_t size, char *fmt, ...);
int fprintf(FILE *stream, char *fmt, ...);

#endif
