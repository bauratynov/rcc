#ifndef _STDLIB_H
#define _STDLIB_H

typedef long long size_t;

void *malloc(size_t size);
void *calloc(size_t nmemb, size_t size);
void *realloc(void *ptr, size_t size);
void free(void *ptr);
void exit(int status);
void abort(void);
int atoi(char *str);
long atol(char *str);
int abs(int n);
double strtod(char *str, char **endptr);
long strtol(char *str, char **endptr, int base);
unsigned long strtoul(char *str, char **endptr, int base);

#endif
