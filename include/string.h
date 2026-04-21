#ifndef _STRING_H
#define _STRING_H

long long strlen(char *s);
char *strcpy(char *dest, char *src);
char *strncpy(char *dest, char *src, long long n);
char *strcat(char *dest, char *src);
int strcmp(char *s1, char *s2);
int strncmp(char *s1, char *s2, long long n);
char *strchr(char *s, int c);
char *strstr(char *haystack, char *needle);
void *memcpy(void *dest, void *src, long long n);
void *memset(void *s, int c, long long n);
int memcmp(void *s1, void *s2, long long n);

#endif
